import { useCallback, useEffect, useRef, useState } from "react";
import { toast } from "sonner";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { openPath } from "@tauri-apps/plugin-opener";
import { Group, Panel, Separator, usePanelRef, type Layout } from "react-resizable-panels";
import { BookmarkDialog } from "@/components/browser/BookmarkDialog";
import { Breadcrumb } from "@/components/browser/Breadcrumb";
import { BrowserFilterBar, type TypeFilter } from "@/components/browser/BrowserFilterBar";
import { BrowserToolbar } from "@/components/browser/BrowserToolbar";
import { ShortcutHelpDialog } from "@/components/browser/ShortcutHelpDialog";
import { BucketPropertiesDialog } from "@/components/browser/BucketPropertiesDialog";
import { BucketSidebar } from "@/components/browser/BucketSidebar";
import { BucketIndexDialog } from "@/components/browser/BucketIndexDialog";
import { BucketIndexSearchDialog } from "@/components/browser/BucketIndexSearchDialog";
import { SizeCalculationDialog } from "@/components/browser/SizeCalculationDialog";
import { CopyMoveDialog, type CopyMoveMode } from "@/components/browser/CopyMoveDialog";
import { DeleteConfirmDialog } from "@/components/browser/DeleteConfirmDialog";
import { FileTable } from "@/components/browser/FileTable";
import { ObjectDetails } from "@/components/browser/ObjectDetails";
import {
  OverwriteDialog,
  type OverwriteConflict,
} from "@/components/browser/OverwriteDialog";
import { LocalFilePanel } from "@/components/browser/LocalFilePanel";
import { ConnectionList } from "@/components/connections/ConnectionList";
import { BucketPromptDialog } from "@/components/connections/BucketPromptDialog";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Button } from "@/components/ui/button";
import { TransferQueue } from "@/components/transfers/TransferQueue";
import { useBrowser, type PrepareUploadResult } from "@/hooks/useBrowser";
import { useConnections } from "@/hooks/useConnections";
import { useKeyboardShortcuts } from "@/hooks/useKeyboardShortcuts";
import { useBucketIndex } from "@/hooks/useBucketIndex";
import { usePrefixSize } from "@/hooks/usePrefixSize";
import { useTransfers } from "@/hooks/useTransfers";
import { useUiState } from "@/hooks/useUiState";
import {
  addBookmark,
  getConnectionNav,
  headObject,
  previewObjectToCache,
  presignObject,
  removeBookmark,
  setConnectionNav,
} from "@/lib/tauri";
import { cn } from "@/lib/utils";
import type { ObjectHeadResponse, PrefixSizeResult, S3Object } from "@/types/s3";
import type { PrefixBookmark } from "@/types/ui";
import { LocalPanelToggle } from "./LocalPanelToggle";
import { ThemeToggle } from "./ThemeToggle";

export function AppShell() {
  const connections = useConnections();
  const browser = useBrowser(connections.selected);
  const transfers = useTransfers();
  const ui = useUiState();
  const prefixSize = usePrefixSize(
    connections.selected?.id ?? null,
    browser.selectedBucket
  );
  const bucketIndex = useBucketIndex(
    connections.selected?.id ?? null,
    browser.selectedBucket
  );

  const [renameOpen, setRenameOpen] = useState(false);
  const [folderOpen, setFolderOpen] = useState(false);
  const [renameValue, setRenameValue] = useState("");
  const [folderValue, setFolderValue] = useState("");
  const [deleteConfirmOpen, setDeleteConfirmOpen] = useState(false);
  const [overwriteOpen, setOverwriteOpen] = useState(false);
  const [pendingUpload, setPendingUpload] = useState<PrepareUploadResult | null>(null);
  const [uploadConflicts, setUploadConflicts] = useState<OverwriteConflict[]>([]);
  const [dragOver, setDragOver] = useState(false);
  const [focusedObject, setFocusedObject] = useState<S3Object | null>(null);
  const [objectDetails, setObjectDetails] = useState<ObjectHeadResponse | null>(null);
  const [detailsLoading, setDetailsLoading] = useState(false);
  const [detailsFromCache, setDetailsFromCache] = useState(false);
  const [renameTarget, setRenameTarget] = useState<S3Object | null>(null);
  const [pendingDelete, setPendingDelete] = useState<S3Object[]>([]);
  const [copyMoveOpen, setCopyMoveOpen] = useState(false);
  const [copyMoveInitialMode, setCopyMoveInitialMode] = useState<CopyMoveMode>("copy");
  const [localPanelOpen, setLocalPanelOpen] = useState(false);
  const [detailsPaneOpen, setDetailsPaneOpen] = useState(true);
  const [connectionsCollapsed, setConnectionsCollapsed] = useState(true);
  const [bucketsCollapsed, setBucketsCollapsed] = useState(true);
  const [uiHydrated, setUiHydrated] = useState(false);
  const [filterText, setFilterText] = useState("");
  const [typeFilter, setTypeFilter] = useState<TypeFilter>("all");
  const [prefixJump, setPrefixJump] = useState("");
  const [shortcutHelpOpen, setShortcutHelpOpen] = useState(false);
  const [bookmarkDialogOpen, setBookmarkDialogOpen] = useState(false);
  const [presignedLoading, setPresignedLoading] = useState(false);
  const [previewPath, setPreviewPath] = useState<string | null>(null);
  const [previewLoading, setPreviewLoading] = useState(false);
  const filterInputRef = useRef<HTMLInputElement>(null);
  const fileListScrollRef = useRef<HTMLDivElement>(null);
  const navRestoredForRef = useRef<string | null>(null);
  const [bucketPropsOpen, setBucketPropsOpen] = useState(false);
  const [sizeCalcOpen, setSizeCalcOpen] = useState(false);
  const [sizeCalcPrefix, setSizeCalcPrefix] = useState("");
  const [sizeCalcTitle, setSizeCalcTitle] = useState("");
  const [bucketSizeResult, setBucketSizeResult] = useState<PrefixSizeResult | null>(null);
  const [bucketIndexOpen, setBucketIndexOpen] = useState(false);
  const [bucketIndexSearchOpen, setBucketIndexSearchOpen] = useState(false);

  const connectionsPanelRef = usePanelRef();
  const bucketsPanelRef = usePanelRef();

  const openRename = (object?: S3Object) => {
    const target = object ?? browser.selectedObjects[0];
    if (!target) return;
    setRenameTarget(target);
    browser.selectKeys([target.key]);
    setRenameValue(target.name.replace(/\/$/, ""));
    setRenameOpen(true);
  };

  const submitRename = async () => {
    if (!renameValue.trim() || !renameTarget) return;
    await browser.renameObjectItem(renameTarget, renameValue.trim());
    setRenameTarget(null);
    setRenameOpen(false);
  };

  const copyObjectPath = (object: S3Object) => {
    const path = browser.selectedBucket
      ? `s3://${browser.selectedBucket}/${object.key}`
      : object.key;
    void navigator.clipboard.writeText(path);
    toast.success("Path copied");
  };

  const submitFolder = async () => {
    if (!folderValue.trim()) return;
    await browser.newFolder(folderValue.trim());
    setFolderValue("");
    setFolderOpen(false);
  };

  const persistDefaultBucket = async (bucket: string) => {
    const connection = connections.selected;
    if (!connection) return;
    await connections.save(
      {
        name: connection.name,
        endpoint: connection.endpoint,
        region: connection.region,
        accessKeyId: connection.accessKeyId,
        secretAccessKey: "",
        forcePathStyle: connection.forcePathStyle,
        defaultBucket: bucket,
      },
      connection.id,
      { quiet: true }
    );
  };

  const handleConnectBucket = async (bucket: string) => {
    await browser.verifyAndConnectBucket(bucket, () => persistDefaultBucket(bucket));
  };

  const browserDisabled = !connections.selected || !browser.selectedBucket;

  const finishUpload = useCallback(
    async (result: PrepareUploadResult) => {
      if (result.items.length === 0) return;

      if (result.conflicts.length === 0) {
        await browser.executeUpload(result.items.map((item) => item.path));
        return;
      }

      setPendingUpload(result);
      setUploadConflicts(
        result.conflicts.map((item) => ({ name: item.name, key: item.key }))
      );
      setOverwriteOpen(true);
    },
    [browser]
  );

  const startUpload = useCallback(
    async (localPaths?: string[]) => {
      if (browserDisabled) return;

      const result = localPaths
        ? await browser.uploadWithPaths(localPaths)
        : await browser.upload();

      await finishUpload(result);
    },
    [browser, browserDisabled, finishUpload]
  );

  const handleOverwriteResolve = async (action: "skip" | "overwrite" | "rename") => {
    if (!pendingUpload) return;

    const conflictKeys = new Set(uploadConflicts.map((conflict) => conflict.key));
    const paths =
      action === "skip"
        ? pendingUpload.items
            .filter((item) => !conflictKeys.has(item.key))
            .map((item) => item.path)
        : pendingUpload.items.map((item) => item.path);

    setPendingUpload(null);
    setUploadConflicts([]);

    if (paths.length > 0) {
      await browser.executeUpload(paths);
    }
  };

  const openDeleteConfirm = (objects?: S3Object[]) => {
    const targets = objects ?? browser.selectedObjects;
    if (targets.length === 0) return;
    setPendingDelete(targets);
    browser.selectKeys(targets.map((object) => object.key));
    setDeleteConfirmOpen(true);
  };

  const confirmDelete = async () => {
    const targets = pendingDelete.length > 0 ? pendingDelete : browser.selectedObjects;
    await browser.removeObjects(targets);
    setPendingDelete([]);
    setDeleteConfirmOpen(false);
  };

  const openCopyMove = (mode: CopyMoveMode) => {
    if (browser.selectedObjects.length === 0) return;
    setCopyMoveInitialMode(mode);
    setCopyMoveOpen(true);
  };

  const handleCopyMoveConfirm = async (
    destBucket: string,
    destPrefix: string,
    mode: CopyMoveMode
  ) => {
    if (mode === "copy") {
      await browser.copySelectedTo(destBucket, destPrefix || undefined);
    } else {
      await browser.moveSelectedTo(destBucket, destPrefix || undefined);
    }
    setCopyMoveOpen(false);
  };

  const handleRowClick = (object: S3Object) => {
    setFocusedObject(object);
    browser.clearSelection();
  };

  const toggleConnectionsPanel = () => {
    const panel = connectionsPanelRef.current;
    if (!panel) return;
    if (panel.isCollapsed()) {
      panel.expand();
      setConnectionsCollapsed(false);
    } else {
      panel.collapse();
      setConnectionsCollapsed(true);
    }
  };

  const toggleBucketsPanel = () => {
    const panel = bucketsPanelRef.current;
    if (!panel) return;
    if (panel.isCollapsed()) {
      panel.expand();
      setBucketsCollapsed(false);
    } else {
      panel.collapse();
      setBucketsCollapsed(true);
    }
  };

  const connectionId = connections.selected?.id ?? null;
  const bookmarks = connectionId ? ui.bookmarksByConnection[connectionId] ?? [] : [];

  const persistPreferences = useCallback(
    (patch: Partial<typeof ui.preferences>) => {
      const next = { ...ui.preferences, ...patch };
      void ui.savePreferences(next);
    },
    [ui]
  );

  const handleLocalPanelToggle = () => {
    setLocalPanelOpen((open) => {
      const next = !open;
      void persistPreferences({ localPanelOpen: next });
      return next;
    });
  };

  const handleDetailsPaneToggle = () => {
    setDetailsPaneOpen((open) => {
      const next = !open;
      void persistPreferences({ detailsPaneOpen: next });
      return next;
    });
  };

  const handleLayoutChanged = useCallback(
    (layout: Layout) => {
      void ui.persistPanelLayout(localPanelOpen ? "four" : "three", layout);
    },
    [ui, localPanelOpen]
  );

  const handlePrefixJumpSubmit = (value: string) => {
    let path = value.trim();
    if (/^s3:\/\//i.test(path)) {
      const withoutScheme = path.replace(/^s3:\/\//i, "");
      const slash = withoutScheme.indexOf("/");
      if (slash === -1) return;
      const bucket = withoutScheme.slice(0, slash);
      path = withoutScheme.slice(slash + 1);
      if (!path.endsWith("/") && path.includes("/")) {
        // keep as file path prefix
      } else if (path && !path.endsWith("/")) {
        path = `${path}/`;
      }
      browser.applyNavigation(bucket, path);
      setPrefixJump("");
      setFilterText("");
      return;
    }

    if (!path.endsWith("/") && path.length > 0) {
      path = `${path}/`;
    }
    browser.navigateToPrefix(path);
    setPrefixJump("");
    setFilterText("");
  };

  const handleBookmarkNavigate = (bucket: string, prefix: string) => {
    browser.applyNavigation(bucket, prefix);
  };

  const handleAddBookmark = () => {
    if (!browser.selectedBucket) return;
    setBookmarkDialogOpen(true);
  };

  const confirmBookmark = async (label: string) => {
    if (!connectionId || !browser.selectedBucket) return;
    const bookmark: PrefixBookmark = {
      id: crypto.randomUUID(),
      label,
      bucket: browser.selectedBucket,
      prefix: browser.prefix,
    };
    await addBookmark(connectionId, bookmark);
    await ui.refreshBookmarks(connectionId);
    toast.success("Bookmark saved");
  };

  const handleRemoveBookmark = async (bookmarkId: string) => {
    if (!connectionId) return;
    await removeBookmark(connectionId, bookmarkId);
    await ui.refreshBookmarks(connectionId);
    toast.success("Bookmark removed");
  };

  const copyToClipboard = async (text: string, message: string) => {
    await navigator.clipboard.writeText(text);
    toast.success(message);
  };

  const handleCopyPresignedUrl = async () => {
    const target = detailsTarget;
    if (!connectionId || !browser.selectedBucket || !target || target.isFolder) return;

    setPresignedLoading(true);
    try {
      const url = await presignObject(connectionId, browser.selectedBucket, target.key);
      await copyToClipboard(url, "Presigned URL copied");
    } catch (error) {
      toast.error("Failed to generate presigned URL", {
        description: error instanceof Error ? error.message : String(error),
      });
    } finally {
      setPresignedLoading(false);
    }
  };

  const handleOpenExternally = async () => {
    if (!previewPath) return;
    try {
      await openPath(previewPath);
    } catch (error) {
      toast.error("Failed to open file", {
        description: error instanceof Error ? error.message : String(error),
      });
    }
  };

  const openSizeCalculation = (prefix: string, title: string) => {
    setSizeCalcPrefix(prefix);
    setSizeCalcTitle(title);
    setSizeCalcOpen(true);
  };

  const openFolderSizeCalc = (object: S3Object) => {
    openSizeCalculation(object.key, `Folder size: ${object.name}`);
  };

  const openBucketSizeCalc = () => {
    if (!browser.selectedBucket) return;
    openSizeCalculation("", `Bucket size: ${browser.selectedBucket}`);
  };

  const handleSizeCalcComplete = (result: PrefixSizeResult) => {
    if (sizeCalcPrefix === "") {
      setBucketSizeResult(result);
    } else {
      prefixSize.seedCache(sizeCalcPrefix, result);
    }
  };

  useKeyboardShortcuts({
    disabled: browserDisabled,
    onRefresh: () => void browser.refreshObjects(),
    onDelete: openDeleteConfirm,
    onUpload: () => void startUpload(),
    onOpenSelected: () => {
      if (focusedObject?.isFolder) {
        browser.openFolder(focusedObject.key);
        return;
      }
      browser.openSelected();
    },
    onSelectAll: () => browser.selectAll(),
    onDownload: () => void browser.download(),
    onFilter: () => filterInputRef.current?.focus(),
    onRename: () => {
      const target = focusedObject ?? browser.selectedObjects[0];
      if (target) openRename(target);
    },
    onHelp: () => setShortcutHelpOpen(true),
  });

  useEffect(() => {
    if (!ui.ready || uiHydrated) return;
    setLocalPanelOpen(ui.preferences.localPanelOpen);
    setDetailsPaneOpen(ui.preferences.detailsPaneOpen);
    setConnectionsCollapsed(ui.preferences.connectionsCollapsed);
    setBucketsCollapsed(ui.preferences.bucketsCollapsed);
    setUiHydrated(true);
  }, [ui.ready, ui.preferences, uiHydrated]);

  useEffect(() => {
    if (!uiHydrated) return;
    const connectionsPanel = connectionsPanelRef.current;
    const bucketsPanel = bucketsPanelRef.current;
    if (connectionsCollapsed) connectionsPanel?.collapse();
    else connectionsPanel?.expand();
    if (bucketsCollapsed) bucketsPanel?.collapse();
    else bucketsPanel?.expand();
  }, [uiHydrated, connectionsCollapsed, bucketsCollapsed]);

  useEffect(() => {
    navRestoredForRef.current = null;
  }, [connectionId]);

  useEffect(() => {
    if (!ui.ready || !connectionId) return;
    if (navRestoredForRef.current === connectionId) return;

    let cancelled = false;
    void getConnectionNav(connectionId).then((nav) => {
      if (cancelled) return;
      navRestoredForRef.current = connectionId;
      if (nav?.bucket) {
        browser.applyNavigation(nav.bucket, nav.prefix ?? "");
      }
      void ui.refreshBookmarks(connectionId);
    });

    return () => {
      cancelled = true;
    };
  }, [ui.ready, connectionId, browser, ui]);

  useEffect(() => {
    if (!connectionId || !browser.selectedBucket) return;
    void setConnectionNav(connectionId, {
      bucket: browser.selectedBucket,
      prefix: browser.prefix,
    });
  }, [connectionId, browser.selectedBucket, browser.prefix]);

  useEffect(() => {
    if (connectionsCollapsed === ui.preferences.connectionsCollapsed) return;
    void persistPreferences({ connectionsCollapsed });
  }, [connectionsCollapsed, ui.preferences.connectionsCollapsed, persistPreferences]);

  useEffect(() => {
    if (bucketsCollapsed === ui.preferences.bucketsCollapsed) return;
    void persistPreferences({ bucketsCollapsed });
  }, [bucketsCollapsed, ui.preferences.bucketsCollapsed, persistPreferences]);

  useEffect(() => {
    setFocusedObject(null);
    setBucketSizeResult(null);
    setFilterText("");
    setPrefixJump("");
    setPreviewPath(null);
  }, [connections.selected?.id, browser.selectedBucket, browser.prefix]);

  useEffect(() => {
    if (browserDisabled) {
      setDragOver(false);
      return;
    }

    let unlisten: (() => void) | undefined;

    void getCurrentWebview()
      .onDragDropEvent((event) => {
        const { type } = event.payload;
        if (type === "enter" || type === "over") {
          setDragOver(true);
        } else if (type === "leave") {
          setDragOver(false);
        } else if (type === "drop") {
          setDragOver(false);
          if (event.payload.paths.length > 0) {
            void startUpload(event.payload.paths);
          }
        }
      })
      .then((fn) => {
        unlisten = fn;
      });

    return () => {
      unlisten?.();
    };
  }, [browserDisabled, startUpload]);

  const detailsTarget =
    browser.selectedObjects.length === 1 ? browser.selectedObjects[0] : focusedObject;

  const needsHeadFetch = !!detailsTarget && !detailsTarget.isFolder;

  useEffect(() => {
    const connection = connections.selected;
    const bucket = browser.selectedBucket;

    if (!connection || !bucket || !needsHeadFetch || !detailsTarget) {
      setObjectDetails(null);
      setDetailsLoading(false);
      setDetailsFromCache(false);
      return;
    }

    let cancelled = false;
    setDetailsLoading(true);
    setObjectDetails(null);
    setDetailsFromCache(false);

    void headObject(connection.id, bucket, detailsTarget.key)
      .then((details) => {
        if (!cancelled) {
          setObjectDetails(details);
          setDetailsFromCache(details.fromCache ?? false);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setObjectDetails(null);
          setDetailsFromCache(false);
        }
      })
      .finally(() => {
        if (!cancelled) setDetailsLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [connections.selected, browser.selectedBucket, detailsTarget?.key, needsHeadFetch]);

  const refreshObjectDetails = useCallback(() => {
    const connection = connections.selected;
    const bucket = browser.selectedBucket;
    if (!connection || !bucket || !needsHeadFetch || !detailsTarget) return;

    setDetailsLoading(true);
    void headObject(connection.id, bucket, detailsTarget.key, true)
      .then((details) => {
        setObjectDetails(details);
        setDetailsFromCache(details.fromCache ?? false);
      })
      .catch(() => {
        setObjectDetails(null);
        setDetailsFromCache(false);
      })
      .finally(() => {
        setDetailsLoading(false);
      });
  }, [
    connections.selected,
    browser.selectedBucket,
    detailsTarget,
    needsHeadFetch,
  ]);

  useEffect(() => {
    const connection = connections.selected;
    const bucket = browser.selectedBucket;
    const target = detailsTarget;

    if (!connection || !bucket || !target || target.isFolder) {
      setPreviewPath(null);
      setPreviewLoading(false);
      return;
    }

    const contentType = objectDetails?.contentType ?? "";
    const canPreview =
      contentType.startsWith("image/") || contentType.startsWith("text/");
    const size = objectDetails?.contentLength ?? target.size;
    if (!canPreview || size > 5 * 1024 * 1024) {
      setPreviewPath(null);
      return;
    }

    let cancelled = false;
    setPreviewLoading(true);
    setPreviewPath(null);

    void previewObjectToCache(connection.id, bucket, target.key)
      .then((path) => {
        if (!cancelled) setPreviewPath(path);
      })
      .catch(() => {
        if (!cancelled) setPreviewPath(null);
      })
      .finally(() => {
        if (!cancelled) setPreviewLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [
    connections.selected,
    browser.selectedBucket,
    detailsTarget?.key,
    detailsTarget?.isFolder,
    objectDetails?.contentType,
    objectDetails?.contentLength,
    detailsTarget?.size,
  ]);

  const folderDetailsKey = detailsTarget?.isFolder ? detailsTarget.key : null;
  const folderSizeActive = folderDetailsKey
    ? prefixSize.getActiveFor(folderDetailsKey)
    : null;

  return (
    <div className="flex h-screen flex-col">
      <header className="flex h-12 items-center justify-between border-b px-4">
        <div className="flex items-center gap-2">
          <div className="flex h-7 w-7 items-center justify-center rounded-md bg-primary text-xs font-bold text-primary-foreground">
            P
          </div>
          <h1 className="text-lg font-semibold tracking-tight">Paker</h1>
        </div>
        <div className="flex items-center gap-1">
          <LocalPanelToggle open={localPanelOpen} onToggle={handleLocalPanelToggle} />
          <ThemeToggle />
        </div>
      </header>

      <div className="flex min-h-0 flex-1 flex-col">
        <Group
          key={localPanelOpen ? "four" : "three"}
          id="paker-main"
          orientation="horizontal"
          className="min-h-0 flex-1"
          defaultLayout={ui.getLayoutForMode(localPanelOpen)}
          onLayoutChanged={handleLayoutChanged}
        >
          <Panel
            id="connections"
            panelRef={connectionsPanelRef}
            collapsible
            collapsedSize={28}
            defaultSize="18%"
            minSize="14%"
            maxSize="35%"
            onResize={() =>
              setConnectionsCollapsed(connectionsPanelRef.current?.isCollapsed() ?? false)
            }
          >
            <ConnectionList
              connections={connections.connections}
              selectedId={connections.selectedId}
              loading={connections.loading}
              testingId={connections.testingId}
              collapsed={connectionsCollapsed}
              onToggleCollapse={toggleConnectionsPanel}
              onSelect={connections.setSelectedId}
              onSave={connections.save}
              onDelete={connections.remove}
              onTest={connections.test}
            />
          </Panel>

          <Separator className="w-1 bg-border" />

          <Panel
            id="buckets"
            panelRef={bucketsPanelRef}
            collapsible
            collapsedSize={28}
            defaultSize="14%"
            minSize="12%"
            maxSize="30%"
            onResize={() => setBucketsCollapsed(bucketsPanelRef.current?.isCollapsed() ?? false)}
          >
            <BucketSidebar
              buckets={browser.buckets}
              selectedBucket={browser.selectedBucket}
              loading={browser.loadingBuckets}
              disabled={!connections.selected}
              collapsed={bucketsCollapsed}
              onToggleCollapse={toggleBucketsPanel}
              onSelect={browser.setSelectedBucket}
              onRefresh={() => void browser.refreshBuckets()}
            />
          </Panel>

          {localPanelOpen && (
            <>
              <Separator className="w-1 bg-border" />

              <Panel id="local" defaultSize="28%" minSize="18%" maxSize="45%">
                <LocalFilePanel
                  connectionId={connections.selected?.id ?? null}
                  onDownloadFromS3={(keys, destDir) => {
                    if (!connections.selected || !browser.selectedBucket) return;
                    const objects = keys.map((k) => ({
                      key: k,
                      name: k.split("/").pop() || k,
                      isFolder: false,
                      size: 0,
                    }));
                    void browser.downloadObjectsTo(objects, destDir);
                  }}
                />
              </Panel>
            </>
          )}

          <Separator className="w-1 bg-border" />

          <Panel
            id="browser"
            defaultSize={localPanelOpen ? "40%" : "58%"}
            minSize="25%"
          >
            <div
              className={cn(
                "flex h-full min-w-0 flex-col transition-shadow",
                dragOver && "ring-2 ring-inset ring-primary"
              )}
            >
              <div className="flex items-center border-b px-3 py-2">
                <span className="text-sm font-semibold text-foreground">Remote</span>
              </div>
              <Breadcrumb
                bucket={browser.selectedBucket}
                segments={browser.breadcrumbs}
                currentPrefix={browser.prefix}
                bookmarks={bookmarks}
                onNavigate={browser.navigateToPrefix}
                onBookmarkNavigate={handleBookmarkNavigate}
                onAddBookmark={handleAddBookmark}
                onRemoveBookmark={handleRemoveBookmark}
              />
              <BrowserFilterBar
                ref={filterInputRef}
                filterText={filterText}
                onFilterChange={setFilterText}
                totalCount={browser.objects.length}
                filteredCount={
                  browser.objects.filter((object) => {
                    const name = object.name.toLowerCase();
                    const query = filterText.trim().toLowerCase();
                    const matchesText = !query || name.includes(query);
                    const matchesType =
                      typeFilter === "all" ||
                      (typeFilter === "folders" && object.isFolder) ||
                      (typeFilter === "files" && !object.isFolder) ||
                      (typeFilter === "glacier" &&
                        object.storageClass?.toLowerCase().includes("glacier"));
                    return matchesText && matchesType;
                  }).length
                }
                typeFilter={typeFilter}
                onTypeFilterChange={setTypeFilter}
                prefixJump={prefixJump}
                onPrefixJump={setPrefixJump}
                onPrefixJumpSubmit={handlePrefixJumpSubmit}
                objectsStale={browser.objectsStale}
                refreshingObjects={browser.refreshingObjects}
              />
              <BrowserToolbar
                disabled={browserDisabled}
                busy={browser.busy}
                hasSelection={browser.selectedObjects.length > 0}
                singleSelection={browser.selectedObjects.length === 1}
                onUpload={() => void startUpload()}
                onDownload={() => void browser.download()}
                onDelete={openDeleteConfirm}
                onRename={openRename}
                onNewFolder={() => setFolderOpen(true)}
                onRefresh={() => void browser.refreshObjects()}
                onCopyTo={() => openCopyMove("copy")}
                onMoveTo={() => openCopyMove("move")}
                onProperties={() => setBucketPropsOpen(true)}
                onCalculateBucketSize={openBucketSizeCalc}
                onIndexBucket={() => setBucketIndexOpen(true)}
                onSearchIndex={() => setBucketIndexSearchOpen(true)}
                indexSearchEnabled={bucketIndex.isSearchable}
              />
              <div className="flex min-h-0 flex-1">
                <div ref={fileListScrollRef} className="min-w-0 flex-1 overflow-auto">
                  <FileTable
                    objects={browser.objects}
                    selectedKeys={browser.selectedKeys}
                    focusedKey={focusedObject?.key ?? null}
                    filterText={filterText}
                    typeFilter={typeFilter}
                    scrollContainerRef={fileListScrollRef}
                    loading={browser.loadingObjects}
                    hasMore={browser.hasMore}
                    loadingMore={browser.loadingMore}
                    onLoadMore={() => void browser.loadMoreObjects()}
                    onToggleKey={browser.toggleKey}
                    onToggleAll={browser.toggleAll}
                    onOpenFolder={browser.openFolder}
                    onRowClick={handleRowClick}
                    onContextUpload={() => void startUpload()}
                    onContextDownload={(objects) => void browser.downloadObjects(objects)}
                    onContextDelete={openDeleteConfirm}
                    onContextRename={openRename}
                    onContextRefresh={() => void browser.refreshObjects()}
                    onContextCopyPath={copyObjectPath}
                    onContextOpen={(object) => browser.openFolder(object.key)}
                    onContextCopyTo={() => openCopyMove("copy")}
                    onContextMoveTo={() => openCopyMove("move")}
                    onContextCalculateSize={openFolderSizeCalc}
                    draggable={!browserDisabled}
                    onDropLocalPaths={(paths) => void startUpload(paths)}
                  />
                </div>
                {detailsPaneOpen ? (
                  <div className="min-w-[220px] w-[28%] max-w-md shrink-0">
                    <ObjectDetails
                      object={detailsTarget}
                      details={objectDetails}
                      loading={detailsLoading && needsHeadFetch}
                      collapsed={false}
                      onToggleCollapse={handleDetailsPaneToggle}
                      selectedObjects={browser.selectedObjects}
                      bucket={browser.selectedBucket}
                      endpoint={connections.selected?.endpoint ?? null}
                      forcePathStyle={connections.selected?.forcePathStyle}
                      connectionId={connectionId}
                      onCopyS3Uri={(uri) => void copyToClipboard(uri, "S3 URI copied")}
                      onCopyHttpsUrl={(url) => void copyToClipboard(url, "HTTPS URL copied")}
                      onCopyPresignedUrl={() => void handleCopyPresignedUrl()}
                      presignedLoading={presignedLoading}
                      previewPath={previewPath}
                      previewLoading={previewLoading}
                      onOpenExternally={() => void handleOpenExternally()}
                      folderSize={
                        folderDetailsKey ? prefixSize.getCached(folderDetailsKey) : null
                      }
                      folderSizeProgress={folderSizeActive?.progress}
                      folderSizeLoading={folderSizeActive?.loading}
                      folderSizeError={folderSizeActive?.error}
                      onCalculateFolderSize={
                        folderDetailsKey
                          ? () => void prefixSize.calculate(folderDetailsKey, { force: true })
                          : undefined
                      }
                      detailsFromCache={detailsFromCache}
                      onRefreshDetails={refreshObjectDetails}
                    />
                  </div>
                ) : (
                  <ObjectDetails
                    object={detailsTarget}
                    details={objectDetails}
                    loading={false}
                    collapsed
                    onToggleCollapse={handleDetailsPaneToggle}
                  />
                )}
              </div>
            </div>
          </Panel>
        </Group>

        <TransferQueue
          transfers={transfers.transfers}
          activeCount={transfers.activeCount}
          onClearCompleted={transfers.clearCompleted}
          onCancel={transfers.cancelTransfer}
          onPause={transfers.pauseTransfer}
          onResume={transfers.resumeTransfer}
        />
      </div>

      <Dialog
        open={renameOpen}
        onOpenChange={(open) => {
          setRenameOpen(open);
          if (!open) setRenameTarget(null);
        }}
      >
        <DialogContent className="max-w-sm">
          <DialogHeader>
            <DialogTitle>Rename</DialogTitle>
          </DialogHeader>
          <div className="space-y-2">
            <Label htmlFor="rename">New name</Label>
            <Input
              id="rename"
              value={renameValue}
              onChange={(e) => setRenameValue(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && void submitRename()}
              autoFocus
            />
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setRenameOpen(false)}>
              Cancel
            </Button>
            <Button onClick={() => void submitRename()}>Rename</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog open={folderOpen} onOpenChange={setFolderOpen}>
        <DialogContent className="max-w-sm">
          <DialogHeader>
            <DialogTitle>New folder</DialogTitle>
          </DialogHeader>
          <div className="space-y-2">
            <Label htmlFor="folder">Folder name</Label>
            <Input
              id="folder"
              value={folderValue}
              onChange={(e) => setFolderValue(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && void submitFolder()}
              autoFocus
            />
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setFolderOpen(false)}>
              Cancel
            </Button>
            <Button onClick={() => void submitFolder()}>Create</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <DeleteConfirmDialog
        open={deleteConfirmOpen}
        onOpenChange={(open) => {
          setDeleteConfirmOpen(open);
          if (!open) setPendingDelete([]);
        }}
        itemCount={
          pendingDelete.length > 0 ? pendingDelete.length : browser.selectedObjects.length
        }
        itemNames={
          pendingDelete.length > 0
            ? pendingDelete.map((object) => object.name)
            : browser.selectedObjects.map((object) => object.name)
        }
        onConfirm={() => void confirmDelete()}
        busy={browser.busy}
      />

      <OverwriteDialog
        open={overwriteOpen}
        onOpenChange={setOverwriteOpen}
        conflicts={uploadConflicts}
        onResolve={(action) => void handleOverwriteResolve(action)}
      />

      <CopyMoveDialog
        open={copyMoveOpen}
        onOpenChange={setCopyMoveOpen}
        buckets={browser.buckets}
        currentBucket={browser.selectedBucket}
        itemCount={browser.selectedObjects.length}
        initialMode={copyMoveInitialMode}
        busy={browser.busy}
        onConfirm={(destBucket, destPrefix, mode) =>
          void handleCopyMoveConfirm(destBucket, destPrefix, mode)
        }
      />

      <BucketPromptDialog
        open={browser.bucketPromptOpen}
        connectionName={connections.selected?.name}
        busy={browser.bucketPromptBusy}
        onOpenChange={browser.setBucketPromptOpen}
        onConnect={handleConnectBucket}
        onListAll={() => browser.browseAllBuckets()}
      />

      <BucketPropertiesDialog
        open={bucketPropsOpen}
        onOpenChange={setBucketPropsOpen}
        connectionId={connections.selected?.id ?? null}
        bucket={browser.selectedBucket}
        onCalculateSize={openBucketSizeCalc}
        sizeResult={bucketSizeResult}
        sizeCalculating={sizeCalcOpen && sizeCalcPrefix === ""}
      />

      <SizeCalculationDialog
        open={sizeCalcOpen}
        onOpenChange={setSizeCalcOpen}
        connectionId={connections.selected?.id ?? null}
        bucket={browser.selectedBucket}
        prefix={sizeCalcPrefix}
        title={sizeCalcTitle}
        onComplete={handleSizeCalcComplete}
      />

      <ShortcutHelpDialog open={shortcutHelpOpen} onOpenChange={setShortcutHelpOpen} />

      <BookmarkDialog
        open={bookmarkDialogOpen}
        onOpenChange={setBookmarkDialogOpen}
        defaultLabel={
          browser.breadcrumbs[browser.breadcrumbs.length - 1]?.label ??
          browser.selectedBucket ??
          "Bookmark"
        }
        onConfirm={(label) => void confirmBookmark(label)}
      />

      <BucketIndexDialog
        open={bucketIndexOpen}
        onOpenChange={setBucketIndexOpen}
        connectionId={connections.selected?.id ?? null}
        bucket={browser.selectedBucket}
        index={bucketIndex}
      />

      <BucketIndexSearchDialog
        open={bucketIndexSearchOpen}
        onOpenChange={setBucketIndexSearchOpen}
        connectionId={connections.selected?.id ?? null}
        bucket={browser.selectedBucket}
        indexStale={bucketIndex.meta?.status === "stale"}
        onNavigate={(prefix) => browser.navigateToPrefix(prefix)}
      />
    </div>
  );
}
