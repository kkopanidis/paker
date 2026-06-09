import { useCallback, useEffect, useMemo, useRef, useState, type RefObject } from "react";
import {
  flexRender,
  getCoreRowModel,
  getSortedRowModel,
  useReactTable,
  type ColumnDef,
  type SortingState,
} from "@tanstack/react-table";
import {
  ArrowDown,
  ArrowUp,
  ArrowUpDown,
  Calculator,
  Copy,
  CopyPlus,
  Download,
  File,
  Folder,
  FolderOpen,
  Loader2,
  MoveRight,
  Pencil,
  RefreshCw,
  Trash2,
  Upload,
} from "lucide-react";
import { Checkbox } from "@/components/ui/checkbox";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuShortcut,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import { Skeleton } from "@/components/ui/skeleton";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { cn } from "@/lib/utils";
import { formatBytes, formatDate } from "@/lib/utils";
import type { S3Object } from "@/types/s3";
import { filterObjects, type TypeFilter } from "@/lib/browser-utils";

interface FileTableProps {
  objects: S3Object[];
  selectedKeys: Set<string>;
  focusedKey?: string | null;
  filterText?: string;
  typeFilter?: TypeFilter;
  scrollContainerRef?: RefObject<HTMLElement | null>;
  loading?: boolean;
  hasMore?: boolean;
  loadingMore?: boolean;
  onLoadMore?: () => void;
  onToggleKey: (key: string, selected: boolean) => void;
  onToggleAll: (selected: boolean) => void;
  onOpenFolder: (key: string) => void;
  onRowClick?: (object: S3Object) => void;
  onContextUpload?: () => void;
  onContextDownload?: (objects: S3Object[]) => void;
  onContextDelete?: (objects: S3Object[]) => void;
  onContextRename?: (object: S3Object) => void;
  onContextRefresh?: () => void;
  onContextCopyPath?: (object: S3Object) => void;
  onContextOpen?: (object: S3Object) => void;
  onContextCopyTo?: () => void;
  onContextMoveTo?: () => void;
  onContextCalculateSize?: (object: S3Object) => void;
  draggable?: boolean;
  onDropLocalPaths?: (paths: string[]) => void;
}

function SortIcon({ sorted }: { sorted: false | "asc" | "desc" }) {
  if (sorted === "asc") return <ArrowUp className="ml-1 inline h-3.5 w-3.5" />;
  if (sorted === "desc") return <ArrowDown className="ml-1 inline h-3.5 w-3.5" />;
  return <ArrowUpDown className="ml-1 inline h-3.5 w-3.5 opacity-40" />;
}

function resolveContextTargets(
  object: S3Object,
  selectedKeys: Set<string>,
  objects: S3Object[]
): S3Object[] {
  if (selectedKeys.has(object.key)) {
    return objects.filter((o) => selectedKeys.has(o.key));
  }
  return [object];
}

function RowContextMenuItems({
  object,
  selectedKeys,
  objects,
  onContextOpen,
  onContextDownload,
  onContextRename,
  onContextCopyPath,
  onContextDelete,
  onContextCopyTo,
  onContextMoveTo,
  onContextCalculateSize,
}: {
  object: S3Object;
  selectedKeys: Set<string>;
  objects: S3Object[];
  onContextOpen?: (object: S3Object) => void;
  onContextDownload?: (objects: S3Object[]) => void;
  onContextRename?: (object: S3Object) => void;
  onContextCopyPath?: (object: S3Object) => void;
  onContextDelete?: (objects: S3Object[]) => void;
  onContextCopyTo?: () => void;
  onContextMoveTo?: () => void;
  onContextCalculateSize?: (object: S3Object) => void;
}) {
  const targets = resolveContextTargets(object, selectedKeys, objects);
  const single = targets.length === 1 ? targets[0] : null;
  const canOpen = !!single?.isFolder && !!onContextOpen;
  const fileTargets = targets.filter((o) => !o.isFolder);
  const canDownload = fileTargets.length > 0 && !!onContextDownload;
  const canRename = !!single && !!onContextRename;
  const canCopyPath = !!single && !!onContextCopyPath;
  const canDelete = targets.length > 0 && !!onContextDelete;
  const canCalculateSize = !!single?.isFolder && !!onContextCalculateSize;

  return (
    <>
      <ContextMenuItem disabled={!canOpen} onSelect={() => single && onContextOpen?.(single)}>
        <FolderOpen className="h-4 w-4" />
        Open
        <ContextMenuShortcut>Enter</ContextMenuShortcut>
      </ContextMenuItem>
      <ContextMenuItem
        disabled={!canCalculateSize}
        onSelect={() => single && onContextCalculateSize?.(single)}
      >
        <Calculator className="h-4 w-4" />
        Calculate size
      </ContextMenuItem>
      <ContextMenuItem
        disabled={!canDownload}
        onSelect={() => onContextDownload?.(fileTargets.length > 0 ? fileTargets : targets)}
      >
        <Download className="h-4 w-4" />
        Download
        <ContextMenuShortcut>⌘D</ContextMenuShortcut>
      </ContextMenuItem>
      <ContextMenuItem disabled={!canRename} onSelect={() => single && onContextRename?.(single)}>
        <Pencil className="h-4 w-4" />
        Rename
        <ContextMenuShortcut>F2</ContextMenuShortcut>
      </ContextMenuItem>
      <ContextMenuItem disabled={!canCopyPath} onSelect={() => single && onContextCopyPath?.(single)}>
        <Copy className="h-4 w-4" />
        Copy path
      </ContextMenuItem>
      {(onContextCopyTo || onContextMoveTo) && <ContextMenuSeparator />}
      {onContextCopyTo && (
        <ContextMenuItem onSelect={() => onContextCopyTo()}>
          <CopyPlus className="h-4 w-4" />
          Copy to…
        </ContextMenuItem>
      )}
      {onContextMoveTo && (
        <ContextMenuItem onSelect={() => onContextMoveTo()}>
          <MoveRight className="h-4 w-4" />
          Move to…
        </ContextMenuItem>
      )}
      <ContextMenuSeparator />
      <ContextMenuItem
        className="text-destructive focus:text-destructive"
        disabled={!canDelete}
        onSelect={() => onContextDelete?.(targets)}
      >
        <Trash2 className="h-4 w-4" />
        Delete
        <ContextMenuShortcut>Del</ContextMenuShortcut>
      </ContextMenuItem>
    </>
  );
}

function AreaContextMenuItems({
  onContextUpload,
  onContextRefresh,
}: {
  onContextUpload?: () => void;
  onContextRefresh?: () => void;
}) {
  return (
    <>
      <ContextMenuItem disabled={!onContextUpload} onSelect={() => onContextUpload?.()}>
        <Upload className="h-4 w-4" />
        Upload
        <ContextMenuShortcut>⌘U</ContextMenuShortcut>
      </ContextMenuItem>
      {onContextRefresh ? (
        <ContextMenuItem onSelect={() => onContextRefresh()}>
          <RefreshCw className="h-4 w-4" />
          Refresh
          <ContextMenuShortcut>F5</ContextMenuShortcut>
        </ContextMenuItem>
      ) : null}
    </>
  );
}

function rowState(selected: boolean, focused: boolean): string | undefined {
  if (selected) return "selected";
  if (focused) return "focused";
  return undefined;
}

export function FileTable({
  objects,
  selectedKeys,
  focusedKey,
  filterText,
  typeFilter = "all",
  scrollContainerRef,
  loading,
  hasMore,
  loadingMore,
  onLoadMore,
  onToggleKey,
  onToggleAll,
  onOpenFolder,
  onRowClick,
  onContextUpload,
  onContextDownload,
  onContextDelete,
  onContextRename,
  onContextRefresh,
  onContextCopyPath,
  onContextOpen,
  onContextCopyTo,
  onContextMoveTo,
  onContextCalculateSize,
  draggable = true,
  onDropLocalPaths,
}: FileTableProps) {
  const [sorting, setSorting] = useState<SortingState>([{ id: "name", desc: false }]);
  const [isDragOver, setIsDragOver] = useState(false);
  const [contextTarget, setContextTarget] = useState<S3Object | null>(null);
  const dragOverRef = useRef(false);
  const loadMoreSentinelRef = useRef<HTMLDivElement>(null);

  const filteredObjects = useMemo(
    () => filterObjects(objects, filterText, typeFilter),
    [objects, filterText, typeFilter]
  );

  const handleRowActivate = useCallback(
    (object: S3Object, event: React.MouseEvent) => {
      if (event.detail === 2 && object.isFolder) {
        event.preventDefault();
        onOpenFolder(object.key);
        return;
      }
      if (event.detail === 1) {
        onRowClick?.(object);
      }
    },
    [onOpenFolder, onRowClick]
  );

  const handleCheckboxCellPointer = useCallback(
    (object: S3Object, event: React.MouseEvent) => {
      if (event.detail === 1) {
        event.stopPropagation();
        return;
      }
      event.stopPropagation();
      if (object.isFolder) {
        event.preventDefault();
        onOpenFolder(object.key);
      }
    },
    [onOpenFolder]
  );

  const handleRowContextMenu = useCallback(
    (object: S3Object) => {
      setContextTarget(object);
      onRowClick?.(object);
    },
    [onRowClick]
  );

  const handleAreaContextMenu = useCallback((event: React.MouseEvent) => {
    const row = (event.target as HTMLElement).closest("tr[data-file-row]");
    if (!row) {
      setContextTarget(null);
    }
  }, []);

  useEffect(() => {
    const sentinel = loadMoreSentinelRef.current;
    if (!sentinel || !hasMore || !onLoadMore) return;

    const observer = new IntersectionObserver(
      (entries) => {
        if (entries[0]?.isIntersecting && hasMore && !loadingMore) {
          onLoadMore();
        }
      },
      {
        root: scrollContainerRef?.current ?? null,
        rootMargin: "120px",
        threshold: 0,
      }
    );

    observer.observe(sentinel);
    return () => observer.disconnect();
  }, [hasMore, loadingMore, onLoadMore, scrollContainerRef, filteredObjects.length]);

  function handleDragStart(e: React.DragEvent, object: S3Object) {
    if (object.isFolder) return;
    const fileKeys = selectedKeys.has(object.key)
      ? objects.filter((o) => selectedKeys.has(o.key) && !o.isFolder).map((o) => o.key)
      : [object.key];
    e.dataTransfer.setData("application/paker-s3-keys", JSON.stringify(fileKeys));
    e.dataTransfer.effectAllowed = "copy";
  }

  function handleDragOver(e: React.DragEvent) {
    if (onDropLocalPaths && e.dataTransfer.types.includes("application/paker-local-paths")) {
      e.preventDefault();
      e.dataTransfer.dropEffect = "copy";
      if (!dragOverRef.current) {
        dragOverRef.current = true;
        setIsDragOver(true);
      }
    }
  }

  function handleDragLeave() {
    dragOverRef.current = false;
    setIsDragOver(false);
  }

  function handleDrop(e: React.DragEvent) {
    dragOverRef.current = false;
    setIsDragOver(false);
    const raw = e.dataTransfer.getData("application/paker-local-paths");
    if (!raw || !onDropLocalPaths) return;
    e.preventDefault();
    try {
      const paths = JSON.parse(raw) as string[];
      if (Array.isArray(paths) && paths.length > 0) {
        onDropLocalPaths(paths);
      }
    } catch {
      // invalid drag data
    }
  }

  const columns = useMemo<ColumnDef<S3Object>[]>(
    () => [
      {
        id: "select",
        header: ({ table }) => (
          <Checkbox
            checked={
              table.getIsAllPageRowsSelected() ||
              (table.getIsSomePageRowsSelected() && "indeterminate")
            }
            onCheckedChange={(value) => onToggleAll(!!value)}
            aria-label="Select all"
          />
        ),
        cell: ({ row }) => {
          const object = row.original;
          return (
            <div
              className="flex h-full min-h-9 items-center"
              onClick={(event) => handleCheckboxCellPointer(object, event)}
              onDoubleClick={(event) => handleCheckboxCellPointer(object, event)}
            >
              <Checkbox
                checked={selectedKeys.has(object.key)}
                onCheckedChange={(value) => onToggleKey(object.key, !!value)}
                aria-label={`Select ${object.name}`}
              />
            </div>
          );
        },
        enableSorting: false,
        size: 40,
      },
      {
        accessorKey: "name",
        header: "Name",
        cell: ({ row }) => {
          const object = row.original;
          return (
            <div className="flex items-center gap-2">
              {object.isFolder ? (
                <Folder className="h-4 w-4 shrink-0 text-amber-500" />
              ) : (
                <File className="h-4 w-4 shrink-0 text-muted-foreground" />
              )}
              <span className="truncate">{object.name}</span>
            </div>
          );
        },
      },
      {
        accessorKey: "size",
        header: "Size",
        cell: ({ row }) =>
          row.original.isFolder ? "—" : formatBytes(row.original.size),
        sortingFn: (a, b) => a.original.size - b.original.size,
      },
      {
        accessorKey: "lastModified",
        header: "Last modified",
        cell: ({ row }) => formatDate(row.original.lastModified),
      },
      {
        accessorKey: "storageClass",
        header: "Storage class",
        cell: ({ row }) => row.original.storageClass ?? "—",
      },
    ],
    [handleCheckboxCellPointer, onToggleAll, onToggleKey, selectedKeys]
  );

  const table = useReactTable({
    data: filteredObjects,
    columns,
    state: { sorting },
    onSortingChange: setSorting,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
  });

  if (loading) {
    return (
      <div className="space-y-2 p-4">
        {Array.from({ length: 8 }).map((_, i) => (
          <Skeleton key={i} className="h-9 w-full" />
        ))}
      </div>
    );
  }

  if (objects.length === 0) {
    return (
      <ContextMenu>
        <ContextMenuTrigger asChild>
          <div
            className={cn(
              "flex h-full min-h-[12rem] items-center justify-center p-8 text-sm text-muted-foreground transition-colors",
              isDragOver && "bg-primary/5 ring-2 ring-inset ring-primary"
            )}
            onDragOver={handleDragOver}
            onDragLeave={handleDragLeave}
            onDrop={handleDrop}
          >
            This folder is empty.
          </div>
        </ContextMenuTrigger>
        <ContextMenuContent>
          <AreaContextMenuItems
            onContextUpload={onContextUpload}
            onContextRefresh={onContextRefresh}
          />
        </ContextMenuContent>
      </ContextMenu>
    );
  }

  if (filteredObjects.length === 0) {
    return (
      <ContextMenu>
        <ContextMenuTrigger asChild>
          <div
            className={cn(
              "flex h-full min-h-[12rem] items-center justify-center p-8 text-sm text-muted-foreground transition-colors",
              isDragOver && "bg-primary/5 ring-2 ring-inset ring-primary"
            )}
            onDragOver={handleDragOver}
            onDragLeave={handleDragLeave}
            onDrop={handleDrop}
          >
            No matching objects.
          </div>
        </ContextMenuTrigger>
        <ContextMenuContent>
          <AreaContextMenuItems
            onContextUpload={onContextUpload}
            onContextRefresh={onContextRefresh}
          />
        </ContextMenuContent>
      </ContextMenu>
    );
  }

  return (
    <ContextMenu
      onOpenChange={(open) => {
        if (!open) setContextTarget(null);
      }}
    >
      <ContextMenuTrigger asChild>
        <div
          className={cn(
            "min-h-full transition-colors",
            isDragOver && "bg-primary/5 ring-2 ring-inset ring-primary"
          )}
          onContextMenu={handleAreaContextMenu}
          onDragOver={handleDragOver}
          onDragLeave={handleDragLeave}
          onDrop={handleDrop}
        >
          <Table wrapScroll={false}>
            <TableHeader className="sticky top-0 z-10 bg-background">
              {table.getHeaderGroups().map((headerGroup) => (
                <TableRow key={headerGroup.id}>
                  {headerGroup.headers.map((header) => (
                    <TableHead key={header.id} style={{ width: header.getSize() }}>
                      {header.isPlaceholder ? null : header.column.getCanSort() ? (
                        <button
                          type="button"
                          className="flex items-center font-medium"
                          onClick={header.column.getToggleSortingHandler()}
                        >
                          {flexRender(header.column.columnDef.header, header.getContext())}
                          <SortIcon sorted={header.column.getIsSorted()} />
                        </button>
                      ) : (
                        flexRender(header.column.columnDef.header, header.getContext())
                      )}
                    </TableHead>
                  ))}
                </TableRow>
              ))}
            </TableHeader>
            <TableBody>
              {table.getRowModel().rows.map((row) => (
                <TableRow
                  key={row.id}
                  data-file-row
                  data-state={rowState(
                    selectedKeys.has(row.original.key),
                    focusedKey === row.original.key
                  )}
                  className="h-9 min-h-9 cursor-pointer"
                  draggable={draggable && !row.original.isFolder}
                  onDragStart={
                    draggable && !row.original.isFolder
                      ? (e) => handleDragStart(e, row.original)
                      : undefined
                  }
                  onClick={(event) => handleRowActivate(row.original, event)}
                  onContextMenu={() => handleRowContextMenu(row.original)}
                >
                  {row.getVisibleCells().map((cell) => (
                    <TableCell key={cell.id}>
                      {flexRender(cell.column.columnDef.cell, cell.getContext())}
                    </TableCell>
                  ))}
                </TableRow>
              ))}
            </TableBody>
          </Table>
          {hasMore ? (
            <div
              ref={loadMoreSentinelRef}
              className="flex justify-center border-t p-3"
              aria-hidden
            >
              {loadingMore ? (
                <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />
              ) : (
                <span className="h-4" />
              )}
            </div>
          ) : null}
        </div>
      </ContextMenuTrigger>
      <ContextMenuContent>
        {contextTarget ? (
          <RowContextMenuItems
            object={contextTarget}
            selectedKeys={selectedKeys}
            objects={filteredObjects}
            onContextOpen={onContextOpen}
            onContextDownload={onContextDownload}
            onContextRename={onContextRename}
            onContextCopyPath={onContextCopyPath}
            onContextDelete={onContextDelete}
            onContextCopyTo={onContextCopyTo}
            onContextMoveTo={onContextMoveTo}
            onContextCalculateSize={onContextCalculateSize}
          />
        ) : (
          <AreaContextMenuItems
            onContextUpload={onContextUpload}
            onContextRefresh={onContextRefresh}
          />
        )}
      </ContextMenuContent>
    </ContextMenu>
  );
}
