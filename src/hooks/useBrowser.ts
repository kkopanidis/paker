import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";
import {
  checkObjectsExist,
  copyObjects,
  createFolder,
  deleteObjects,
  downloadFiles,
  formatIpcError,
  listBuckets,
  listObjects,
  moveObjects,
  normalizeObjects,
  pickUploadFiles,
  readListCache,
  renameObject,
  uploadFiles,
  verifyBucket,
} from "@/lib/tauri";
import type { S3Connection } from "@/types/connection";
import type { BucketInfo, S3Object } from "@/types/s3";

function parentPrefix(prefix: string): string {
  const trimmed = prefix.replace(/\/$/, "");
  if (!trimmed) return "";
  const idx = trimmed.lastIndexOf("/");
  return idx === -1 ? "" : `${trimmed.slice(0, idx + 1)}`;
}

function prefixSegments(prefix: string): { label: string; path: string }[] {
  if (!prefix) return [];
  const parts = prefix.replace(/\/$/, "").split("/").filter(Boolean);
  return parts.map((part, index) => ({
    label: part,
    path: `${parts.slice(0, index + 1).join("/")}/`,
  }));
}

function joinPrefix(prefix: string, fileName: string): string {
  const base = fileName.split(/[/\\]/).pop() || fileName;
  let key = prefix;
  if (key && !key.endsWith("/")) key += "/";
  return `${key}${base}`;
}

export interface UploadItem {
  path: string;
  key: string;
  name: string;
}

export interface PrepareUploadResult {
  items: UploadItem[];
  conflicts: UploadItem[];
}

function fixedBucket(name: string): BucketInfo[] {
  return [{ name }];
}

export function useBrowser(connection: S3Connection | null) {
  const [buckets, setBuckets] = useState<BucketInfo[]>([]);
  const [selectedBucket, setSelectedBucket] = useState<string | null>(null);
  const [prefix, setPrefix] = useState("");
  const [objects, setObjects] = useState<S3Object[]>([]);
  const [selectedKeys, setSelectedKeys] = useState<Set<string>>(new Set());
  const [loadingBuckets, setLoadingBuckets] = useState(false);
  const [loadingObjects, setLoadingObjects] = useState(false);
  const [continuationToken, setContinuationToken] = useState<string | null>(null);
  const [hasMore, setHasMore] = useState(false);
  const [loadingMore, setLoadingMore] = useState(false);
  const [objectsStale, setObjectsStale] = useState(false);
  const [objectsFetchedAt, setObjectsFetchedAt] = useState<string | null>(null);
  const [refreshingObjects, setRefreshingObjects] = useState(false);
  const [busy, setBusy] = useState(false);
  const [bucketPromptOpen, setBucketPromptOpen] = useState(false);
  const [bucketPromptBusy, setBucketPromptBusy] = useState(false);

  const configuredBucket = connection?.defaultBucket?.trim() || null;
  const loadGenerationRef = useRef(0);
  const navigationRef = useRef<{ connectionId: string; bucket: string } | null>(null);

  const applyFixedBucket = useCallback((name: string) => {
    if (!connection) return;
    setBucketPromptOpen(false);
    setBuckets(fixedBucket(name));
    navigationRef.current = { connectionId: connection.id, bucket: name };
    setSelectedBucket(name);
  }, [connection]);

  const loadBuckets = useCallback(async () => {
    if (!connection) {
      setBuckets([]);
      navigationRef.current = null;
      setSelectedBucket(null);
      setBucketPromptOpen(false);
      return;
    }

    if (configuredBucket) {
      setLoadingBuckets(true);
      try {
        applyFixedBucket(configuredBucket);
      } finally {
        setLoadingBuckets(false);
      }
      return;
    }

    setBuckets([]);
    navigationRef.current = null;
    setSelectedBucket(null);
    setBucketPromptOpen(true);
  }, [connection, configuredBucket, applyFixedBucket]);

  const browseAllBuckets = useCallback(async () => {
    if (!connection) return;

    setBucketPromptBusy(true);
    setLoadingBuckets(true);
    try {
      const result = await listBuckets(connection.id, true);
      setBuckets(result);
      const bucket = result[0]?.name ?? null;
      navigationRef.current = bucket
        ? { connectionId: connection.id, bucket }
        : null;
      setSelectedBucket(bucket);
      setBucketPromptOpen(false);
      if (result.length === 0) {
        toast.message("No buckets found");
      }
    } catch (error) {
      toast.error("Failed to list buckets", {
        description: formatIpcError(error),
      });
    } finally {
      setBucketPromptBusy(false);
      setLoadingBuckets(false);
    }
  }, [connection]);

  const verifyAndConnectBucket = useCallback(
    async (bucket: string, persist: () => Promise<void>) => {
      if (!connection) return;

      setBucketPromptBusy(true);
      try {
        await verifyBucket(connection.id, bucket);
        await persist();
        applyFixedBucket(bucket);
        toast.success("Connected to bucket", { description: bucket });
      } catch (error) {
        toast.error("Could not access bucket", {
          description: formatIpcError(error),
        });
        throw error;
      } finally {
        setBucketPromptBusy(false);
      }
    },
    [connection, applyFixedBucket]
  );

  const loadObjects = useCallback(async (forceRefresh = false) => {
    const generation = loadGenerationRef.current;

    const navigation = navigationRef.current;
    if (
      !connection ||
      !selectedBucket ||
      !navigation ||
      navigation.connectionId !== connection.id ||
      navigation.bucket !== selectedBucket
    ) {
      setObjects([]);
      setContinuationToken(null);
      setHasMore(false);
      setObjectsStale(false);
      setObjectsFetchedAt(null);
      setRefreshingObjects(false);
      return;
    }

    setContinuationToken(null);
    setHasMore(false);

    let cacheHit = false;
    if (!forceRefresh) {
      try {
        const cached = await readListCache(connection.id, selectedBucket, prefix);
        if (generation !== loadGenerationRef.current) return;
        if (cached) {
          cacheHit = true;
          setObjects(normalizeObjects(cached.result, prefix));
          setSelectedKeys(new Set());
          setContinuationToken(cached.result.continuationToken ?? null);
          setHasMore(cached.result.isTruncated);
          setObjectsStale(true);
          setObjectsFetchedAt(cached.fetchedAt);
          setLoadingObjects(false);
        }
      } catch {
        // Ignore cache read errors; fall through to network fetch.
      }
    }

    if (generation !== loadGenerationRef.current) return;

    if (!cacheHit) {
      setLoadingObjects(true);
      if (forceRefresh) {
        setObjectsStale(false);
      }
    }

    setRefreshingObjects(true);
    try {
      const result = await listObjects(
        connection.id,
        selectedBucket,
        prefix,
        undefined,
        forceRefresh
      );
      if (generation !== loadGenerationRef.current) return;
      setObjects(normalizeObjects(result, prefix));
      setSelectedKeys(new Set());
      setContinuationToken(result.continuationToken ?? null);
      setHasMore(result.isTruncated);
      setObjectsStale(false);
      setObjectsFetchedAt(result.fetchedAt ?? null);
    } catch (error) {
      if (generation !== loadGenerationRef.current) return;
      if (!cacheHit) {
        toast.error("Failed to list objects", {
          description: formatIpcError(error),
        });
      }
    } finally {
      if (generation === loadGenerationRef.current) {
        setLoadingObjects(false);
        setRefreshingObjects(false);
      }
    }
  }, [connection, selectedBucket, prefix]);

  const refreshObjects = useCallback(() => loadObjects(true), [loadObjects]);

  const loadMoreObjects = useCallback(async () => {
    if (
      !hasMore ||
      loadingMore ||
      !connection ||
      !selectedBucket ||
      !continuationToken ||
      navigationRef.current?.connectionId !== connection.id ||
      navigationRef.current?.bucket !== selectedBucket
    ) {
      return;
    }

    const generation = loadGenerationRef.current;
    setLoadingMore(true);
    try {
      const result = await listObjects(
        connection.id,
        selectedBucket,
        prefix,
        continuationToken
      );
      if (generation !== loadGenerationRef.current) return;
      const page = normalizeObjects(result, prefix);
      setObjects((current) => {
        const seen = new Set(current.map((o) => o.key));
        const merged = [...current];
        for (const object of page) {
          if (!seen.has(object.key)) {
            seen.add(object.key);
            merged.push(object);
          }
        }
        return merged.sort((a, b) => {
          if (a.isFolder !== b.isFolder) return a.isFolder ? -1 : 1;
          return a.name.localeCompare(b.name);
        });
      });
      setContinuationToken(result.continuationToken ?? null);
      setHasMore(result.isTruncated);
    } catch (error) {
      if (generation !== loadGenerationRef.current) return;
      toast.error("Failed to load more objects", {
        description: formatIpcError(error),
      });
    } finally {
      if (generation === loadGenerationRef.current) {
        setLoadingMore(false);
      }
    }
  }, [hasMore, loadingMore, connection, selectedBucket, prefix, continuationToken]);

  const setSelectedBucketForConnection = useCallback(
    (bucket: string | null) => {
      navigationRef.current =
        bucket && connection ? { connectionId: connection.id, bucket } : null;
      setSelectedBucket(bucket);
    },
    [connection]
  );

  useLayoutEffect(() => {
    loadGenerationRef.current += 1;
    navigationRef.current = null;
    setSelectedBucket(null);
    setPrefix("");
    setObjects([]);
    setSelectedKeys(new Set());
    setContinuationToken(null);
    setHasMore(false);
    setObjectsStale(false);
    setObjectsFetchedAt(null);
    void loadBuckets();
  }, [connection?.id, configuredBucket, loadBuckets]);

  const applyNavigation = useCallback(
    (bucket: string, nextPrefix: string) => {
      if (connection) {
        navigationRef.current = { connectionId: connection.id, bucket };
      }
      setSelectedBucket(bucket);
      if (!configuredBucket) {
        setBuckets((current) =>
          current.some((entry) => entry.name === bucket)
            ? current
            : [...current, { name: bucket }]
        );
      }
      setPrefix(nextPrefix);
      setSelectedKeys(new Set());
      setContinuationToken(null);
      setHasMore(false);
    },
    [configuredBucket, connection]
  );

  useEffect(() => {
    void loadObjects();
  }, [loadObjects]);

  const navigateToPrefix = useCallback((nextPrefix: string) => {
    setPrefix(nextPrefix);
    setSelectedKeys(new Set());
    setContinuationToken(null);
    setHasMore(false);
  }, []);

  const navigateUp = useCallback(() => {
    setPrefix(parentPrefix(prefix));
    setSelectedKeys(new Set());
  }, [prefix]);

  const openFolder = useCallback(
    (key: string) => {
      navigateToPrefix(key.endsWith("/") ? key : `${key}/`);
    },
    [navigateToPrefix]
  );

  const breadcrumbs = useMemo(() => prefixSegments(prefix), [prefix]);

  const toggleKey = useCallback((key: string, selected: boolean) => {
    setSelectedKeys((current) => {
      const next = new Set(current);
      if (selected) next.add(key);
      else next.delete(key);
      return next;
    });
  }, []);

  const toggleAll = useCallback(
    (selected: boolean) => {
      if (selected) {
        setSelectedKeys(new Set(objects.map((o) => o.key)));
      } else {
        setSelectedKeys(new Set());
      }
    },
    [objects]
  );

  const selectKeys = useCallback((keys: string[]) => {
    setSelectedKeys(new Set(keys));
  }, []);

  const selectAll = useCallback(() => toggleAll(true), [toggleAll]);

  const clearSelection = useCallback(() => {
    setSelectedKeys(new Set());
  }, []);

  const selectedObjects = useMemo(
    () => objects.filter((o) => selectedKeys.has(o.key)),
    [objects, selectedKeys]
  );

  const openSelected = useCallback(() => {
    if (selectedObjects.length !== 1) return;
    const object = selectedObjects[0];
    if (object.isFolder) {
      openFolder(object.key);
    }
  }, [selectedObjects, openFolder]);

  const runAction = useCallback(
    async (label: string, action: () => Promise<unknown>) => {
      setBusy(true);
      try {
        await action();
        toast.success(label);
        await loadObjects(true);
      } catch (error) {
        toast.error(`${label} failed`, {
          description: formatIpcError(error),
        });
      } finally {
        setBusy(false);
      }
    },
    [loadObjects]
  );

  const prepareUpload = useCallback(
    async (localPaths: string[]): Promise<PrepareUploadResult> => {
      if (!connection || !selectedBucket || localPaths.length === 0) {
        return { items: [], conflicts: [] };
      }

      const items: UploadItem[] = localPaths.map((path) => {
        const name = path.split(/[/\\]/).pop() || path;
        const key = joinPrefix(prefix, name);
        return { path, key, name };
      });

      const existingKeys = new Set(
        await checkObjectsExist(
          connection.id,
          selectedBucket,
          items.map((item) => item.key)
        )
      );

      const conflicts = items.filter((item) => existingKeys.has(item.key));
      return { items, conflicts };
    },
    [connection, selectedBucket, prefix]
  );

  const executeUpload = useCallback(
    async (localPaths: string[]) => {
      if (!connection || !selectedBucket || localPaths.length === 0) return;
      await runAction("Upload started", () =>
        uploadFiles(connection.id, selectedBucket, prefix, localPaths)
      );
    },
    [connection, selectedBucket, prefix, runAction]
  );

  const upload = useCallback(async () => {
    const paths = await pickUploadFiles();
    return prepareUpload(paths);
  }, [prepareUpload]);

  const uploadWithPaths = useCallback(
    async (localPaths: string[]) => prepareUpload(localPaths),
    [prepareUpload]
  );

  const downloadObjects = useCallback(
    async (targets: S3Object[]) => {
      if (!connection || !selectedBucket || targets.length === 0) return;
      const fileKeys = targets.filter((o) => !o.isFolder).map((o) => o.key);
      if (fileKeys.length === 0) {
        toast.error("Select at least one file to download");
        return;
      }
      await runAction("Download started", () =>
        downloadFiles(connection.id, selectedBucket, fileKeys)
      );
    },
    [connection, selectedBucket, runAction]
  );

  const downloadObjectsTo = useCallback(
    async (targets: S3Object[], saveDir: string) => {
      if (!connection || !selectedBucket || targets.length === 0) return;
      const fileKeys = targets.filter((o) => !o.isFolder).map((o) => o.key);
      if (fileKeys.length === 0) {
        toast.error("Select at least one file to download");
        return;
      }
      await runAction("Download started", () =>
        downloadFiles(connection.id, selectedBucket, fileKeys, saveDir)
      );
    },
    [connection, selectedBucket, runAction]
  );

  const download = useCallback(async () => {
    await downloadObjects(selectedObjects);
  }, [downloadObjects, selectedObjects]);

  const removeObjects = useCallback(
    async (targets: S3Object[]) => {
      if (!connection || !selectedBucket || targets.length === 0) return;
      await runAction("Delete completed", () =>
        deleteObjects(
          connection.id,
          selectedBucket,
          targets.map((o) => o.key)
        )
      );
    },
    [connection, selectedBucket, runAction]
  );

  const removeSelected = useCallback(async () => {
    await removeObjects(selectedObjects);
  }, [removeObjects, selectedObjects]);

  const renameObjectItem = useCallback(
    async (object: S3Object, newName: string) => {
      if (!connection || !selectedBucket) return;
      const newKey = `${prefix}${newName}${object.isFolder ? "/" : ""}`;
      await runAction("Rename completed", () =>
        renameObject(connection.id, selectedBucket, object.key, newKey)
      );
    },
    [connection, selectedBucket, prefix, runAction]
  );

  const renameSelected = useCallback(
    async (newName: string) => {
      if (selectedObjects.length !== 1) return;
      await renameObjectItem(selectedObjects[0], newName);
    },
    [renameObjectItem, selectedObjects]
  );

  const newFolder = useCallback(
    async (name: string) => {
      if (!connection || !selectedBucket) return;
      await runAction("Folder created", () =>
        createFolder(connection.id, selectedBucket, prefix, name)
      );
    },
    [connection, selectedBucket, prefix, runAction]
  );

  const copySelectedTo = useCallback(
    async (destBucket: string, destPrefix?: string) => {
      if (!connection || !selectedBucket || selectedKeys.size === 0) return;
      const items = [...selectedKeys].map((key) => ({ srcKey: key }));
      await runAction("Copy completed", () =>
        copyObjects(connection.id, selectedBucket, destBucket, items, destPrefix)
      );
    },
    [connection, selectedBucket, selectedKeys, runAction]
  );

  const moveSelectedTo = useCallback(
    async (destBucket: string, destPrefix?: string) => {
      if (!connection || !selectedBucket || selectedKeys.size === 0) return;
      const items = [...selectedKeys].map((key) => ({ srcKey: key }));
      await runAction("Move completed", () =>
        moveObjects(connection.id, selectedBucket, destBucket, items, destPrefix)
      );
    },
    [connection, selectedBucket, selectedKeys, runAction]
  );

  return {
    buckets,
    selectedBucket,
    setSelectedBucket: setSelectedBucketForConnection,
    prefix,
    breadcrumbs,
    objects,
    selectedKeys,
    selectedObjects,
    loadingBuckets,
    loadingObjects,
    objectsStale,
    objectsFetchedAt,
    refreshingObjects,
    hasMore,
    loadingMore,
    busy,
    bucketPromptOpen,
    setBucketPromptOpen,
    bucketPromptBusy,
    browseAllBuckets,
    verifyAndConnectBucket,
    applyNavigation,
    navigateToPrefix,
    navigateUp,
    openFolder,
    toggleKey,
    toggleAll,
    selectKeys,
    selectAll,
    clearSelection,
    refreshBuckets: loadBuckets,
    refreshObjects,
    loadMoreObjects,
    prepareUpload,
    executeUpload,
    upload,
    uploadWithPaths,
    openSelected,
    download,
    downloadObjects,
    downloadObjectsTo,
    removeSelected,
    removeObjects,
    renameSelected,
    renameObjectItem,
    newFolder,
    copySelectedTo,
    moveSelectedTo,
  };
}
