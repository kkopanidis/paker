import { useCallback, useEffect, useState } from "react";
import { toast } from "sonner";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { Group, Panel, Separator } from "react-resizable-panels";
import { Breadcrumb } from "@/components/browser/Breadcrumb";
import { BrowserToolbar } from "@/components/browser/BrowserToolbar";
import { BucketSidebar } from "@/components/browser/BucketSidebar";
import { DeleteConfirmDialog } from "@/components/browser/DeleteConfirmDialog";
import { FileTable } from "@/components/browser/FileTable";
import { ObjectDetails } from "@/components/browser/ObjectDetails";
import {
  OverwriteDialog,
  type OverwriteConflict,
} from "@/components/browser/OverwriteDialog";
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
import { ScrollArea } from "@/components/ui/scroll-area";
import { TransferQueue } from "@/components/transfers/TransferQueue";
import { useBrowser, type PrepareUploadResult } from "@/hooks/useBrowser";
import { useConnections } from "@/hooks/useConnections";
import { useKeyboardShortcuts } from "@/hooks/useKeyboardShortcuts";
import { useTransfers } from "@/hooks/useTransfers";
import { headObject } from "@/lib/tauri";
import { cn } from "@/lib/utils";
import type { ObjectHeadDetails, S3Object } from "@/types/s3";
import { ThemeToggle } from "./ThemeToggle";

export function AppShell() {
  const connections = useConnections();
  const browser = useBrowser(connections.selected);
  const transfers = useTransfers();

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
  const [objectDetails, setObjectDetails] = useState<ObjectHeadDetails | null>(null);
  const [detailsLoading, setDetailsLoading] = useState(false);
  const [renameTarget, setRenameTarget] = useState<S3Object | null>(null);
  const [pendingDelete, setPendingDelete] = useState<S3Object[]>([]);

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

  useKeyboardShortcuts({
    disabled: browserDisabled,
    onRefresh: () => void browser.refreshObjects(),
    onDelete: openDeleteConfirm,
    onUpload: () => void startUpload(),
    onOpenSelected: () => browser.openSelected(),
    onSelectAll: () => browser.selectAll(),
    onDownload: () => void browser.download(),
  });

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

  const detailObject =
    browser.selectedObjects.length === 1 && !browser.selectedObjects[0].isFolder
      ? browser.selectedObjects[0]
      : null;

  useEffect(() => {
    const connection = connections.selected;
    const bucket = browser.selectedBucket;

    if (!connection || !bucket || !detailObject) {
      setObjectDetails(null);
      setDetailsLoading(false);
      return;
    }

    let cancelled = false;
    setDetailsLoading(true);
    setObjectDetails(null);

    void headObject(connection.id, bucket, detailObject.key)
      .then((details) => {
        if (!cancelled) setObjectDetails(details);
      })
      .catch(() => {
        if (!cancelled) setObjectDetails(null);
      })
      .finally(() => {
        if (!cancelled) setDetailsLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [connections.selected, browser.selectedBucket, detailObject?.key]);

  return (
    <div className="flex h-screen flex-col">
      <header className="flex h-12 items-center justify-between border-b px-4">
        <div className="flex items-center gap-2">
          <div className="flex h-7 w-7 items-center justify-center rounded-md bg-primary text-xs font-bold text-primary-foreground">
            P
          </div>
          <h1 className="text-lg font-semibold tracking-tight">Paker</h1>
        </div>
        <ThemeToggle />
      </header>

      <div className="flex min-h-0 flex-1 flex-col">
        <Group
          id="paker-main"
          orientation="horizontal"
          className="min-h-0 flex-1"
          defaultLayout={{ connections: 22, buckets: 20, browser: 58 }}
        >
          <Panel id="connections" defaultSize="22%" minSize="14%" maxSize="35%">
            <ConnectionList
              connections={connections.connections}
              selectedId={connections.selectedId}
              loading={connections.loading}
              testingId={connections.testingId}
              onSelect={connections.setSelectedId}
              onSave={connections.save}
              onDelete={connections.remove}
              onTest={connections.test}
            />
          </Panel>

          <Separator className="w-1 bg-border" />

          <Panel id="buckets" defaultSize="20%" minSize="12%" maxSize="30%">
            <BucketSidebar
              buckets={browser.buckets}
              selectedBucket={browser.selectedBucket}
              loading={browser.loadingBuckets}
              disabled={!connections.selected}
              onSelect={browser.setSelectedBucket}
              onRefresh={() => void browser.refreshBuckets()}
            />
          </Panel>

          <Separator className="w-1 bg-border" />

          <Panel id="browser" defaultSize="58%" minSize="35%">
            <div
              className={cn(
                "flex h-full min-w-0 flex-col transition-shadow",
                dragOver && "ring-2 ring-inset ring-primary"
              )}
            >
              <Breadcrumb
                bucket={browser.selectedBucket}
                segments={browser.breadcrumbs}
                onNavigate={browser.navigateToPrefix}
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
              />
              <div className="flex min-h-0 flex-1">
                <ScrollArea className="min-w-0 flex-[3]">
                  <FileTable
                    objects={browser.objects}
                    selectedKeys={browser.selectedKeys}
                    loading={browser.loadingObjects}
                    hasMore={browser.hasMore}
                    loadingMore={browser.loadingMore}
                    onLoadMore={() => void browser.loadMoreObjects()}
                    onToggleKey={browser.toggleKey}
                    onToggleAll={browser.toggleAll}
                    onOpenFolder={browser.openFolder}
                    onRowClick={setFocusedObject}
                    onContextUpload={() => void startUpload()}
                    onContextDownload={(objects) => void browser.downloadObjects(objects)}
                    onContextDelete={openDeleteConfirm}
                    onContextRename={openRename}
                    onContextRefresh={() => void browser.refreshObjects()}
                    onContextCopyPath={copyObjectPath}
                    onContextOpen={(object) => browser.openFolder(object.key)}
                  />
                </ScrollArea>
                <div className="min-w-[200px] flex-1">
                  <ObjectDetails
                    object={detailObject ?? focusedObject}
                    details={objectDetails}
                    loading={detailsLoading && !!detailObject}
                  />
                </div>
              </div>
            </div>
          </Panel>
        </Group>

        <TransferQueue
          transfers={transfers.transfers}
          activeCount={transfers.activeCount}
          onClearCompleted={transfers.clearCompleted}
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

      <BucketPromptDialog
        open={browser.bucketPromptOpen}
        connectionName={connections.selected?.name}
        busy={browser.bucketPromptBusy}
        onOpenChange={browser.setBucketPromptOpen}
        onConnect={handleConnectBucket}
        onListAll={() => browser.browseAllBuckets()}
      />
    </div>
  );
}
