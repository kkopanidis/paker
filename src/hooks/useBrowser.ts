import { useCallback, useEffect, useMemo, useState } from "react";
import { toast } from "sonner";
import {
  checkObjectsExist,
  copyObjects,
  createFolder,
  deleteObjects,
  downloadFiles,
  listBuckets,
  listObjects,
  moveObjects,
  normalizeObjects,
  pickUploadFiles,
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
  const [busy, setBusy] = useState(false);
  const [bucketPromptOpen, setBucketPromptOpen] = useState(false);
  const [bucketPromptBusy, setBucketPromptBusy] = useState(false);

  const configuredBucket = connection?.defaultBucket?.trim() || null;

  const applyFixedBucket = useCallback((name: string) => {
    setBucketPromptOpen(false);
    setBuckets(fixedBucket(name));
    setSelectedBucket(name);
  }, []);

  const loadBuckets = useCallback(async () => {
    if (!connection) {
      setBuckets([]);
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
      setSelectedBucket(result[0]?.name ?? null);
      setBucketPromptOpen(false);
      if (result.length === 0) {
        toast.message("No buckets found");
      }
    } catch (error) {
      toast.error("Failed to list buckets", {
        description: error instanceof Error ? error.message : String(error),
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
          description: error instanceof Error ? error.message : String(error),
        });
        throw error;
      } finally {
        setBucketPromptBusy(false);
      }
    },
    [connection, applyFixedBucket]
  );

  const loadObjects = useCallback(async () => {
    if (!connection || !selectedBucket) {
      setObjects([]);
      setContinuationToken(null);
      setHasMore(false);
      return;
    }

    setLoadingObjects(true);
    setContinuationToken(null);
    setHasMore(false);
    try {
      const result = await listObjects(connection.id, selectedBucket, prefix);
      setObjects(normalizeObjects(result, prefix));
      setSelectedKeys(new Set());
      setContinuationToken(result.continuationToken ?? null);
      setHasMore(result.isTruncated);
    } catch (error) {
      toast.error("Failed to list objects", {
        description: error instanceof Error ? error.message : String(error),
      });
    } finally {
      setLoadingObjects(false);
    }
  }, [connection, selectedBucket, prefix]);

  const loadMoreObjects = useCallback(async () => {
    if (!hasMore || loadingMore || !connection || !selectedBucket || !continuationToken) {
      return;
    }

    setLoadingMore(true);
    try {
      const result = await listObjects(
        connection.id,
        selectedBucket,
        prefix,
        continuationToken
      );
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
      toast.error("Failed to load more objects", {
        description: error instanceof Error ? error.message : String(error),
      });
    } finally {
      setLoadingMore(false);
    }
  }, [hasMore, loadingMore, connection, selectedBucket, prefix, continuationToken]);

  useEffect(() => {
    void loadBuckets();
    setPrefix("");
    setSelectedKeys(new Set());
  }, [connection?.id, configuredBucket, loadBuckets]);

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
        await loadObjects();
      } catch (error) {
        toast.error(`${label} failed`, {
          description: error instanceof Error ? error.message : String(error),
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
    setSelectedBucket,
    prefix,
    breadcrumbs,
    objects,
    selectedKeys,
    selectedObjects,
    loadingBuckets,
    loadingObjects,
    hasMore,
    loadingMore,
    busy,
    bucketPromptOpen,
    setBucketPromptOpen,
    bucketPromptBusy,
    browseAllBuckets,
    verifyAndConnectBucket,
    navigateToPrefix,
    navigateUp,
    openFolder,
    toggleKey,
    toggleAll,
    selectKeys,
    selectAll,
    refreshBuckets: loadBuckets,
    refreshObjects: loadObjects,
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
