import { useMemo, useState } from "react";
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
  Copy,
  Download,
  File,
  Folder,
  FolderOpen,
  Loader2,
  Pencil,
  RefreshCw,
  Trash2,
  Upload,
} from "lucide-react";
import { Button } from "@/components/ui/button";
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
import { formatBytes, formatDate } from "@/lib/utils";
import type { S3Object } from "@/types/s3";

interface FileTableProps {
  objects: S3Object[];
  selectedKeys: Set<string>;
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

function RowContextMenu({
  object,
  selectedKeys,
  objects,
  onContextOpen,
  onContextDownload,
  onContextRename,
  onContextCopyPath,
  onContextDelete,
}: {
  object: S3Object;
  selectedKeys: Set<string>;
  objects: S3Object[];
  onContextOpen?: (object: S3Object) => void;
  onContextDownload?: (objects: S3Object[]) => void;
  onContextRename?: (object: S3Object) => void;
  onContextCopyPath?: (object: S3Object) => void;
  onContextDelete?: (objects: S3Object[]) => void;
}) {
  const targets = resolveContextTargets(object, selectedKeys, objects);
  const single = targets.length === 1 ? targets[0] : null;
  const canOpen = !!single?.isFolder && !!onContextOpen;
  const fileTargets = targets.filter((o) => !o.isFolder);
  const canDownload = fileTargets.length > 0 && !!onContextDownload;
  const canRename = !!single && !!onContextRename;
  const canCopyPath = !!single && !!onContextCopyPath;
  const canDelete = targets.length > 0 && !!onContextDelete;

  return (
    <ContextMenuContent>
      <ContextMenuItem disabled={!canOpen} onSelect={() => single && onContextOpen?.(single)}>
        <FolderOpen className="h-4 w-4" />
        Open
      </ContextMenuItem>
      <ContextMenuItem
        disabled={!canDownload}
        onSelect={() => onContextDownload?.(fileTargets.length > 0 ? fileTargets : targets)}
      >
        <Download className="h-4 w-4" />
        Download
      </ContextMenuItem>
      <ContextMenuItem disabled={!canRename} onSelect={() => single && onContextRename?.(single)}>
        <Pencil className="h-4 w-4" />
        Rename
      </ContextMenuItem>
      <ContextMenuItem disabled={!canCopyPath} onSelect={() => single && onContextCopyPath?.(single)}>
        <Copy className="h-4 w-4" />
        Copy path
      </ContextMenuItem>
      <ContextMenuSeparator />
      <ContextMenuItem
        className="text-destructive focus:text-destructive"
        disabled={!canDelete}
        onSelect={() => onContextDelete?.(targets)}
      >
        <Trash2 className="h-4 w-4" />
        Delete
      </ContextMenuItem>
    </ContextMenuContent>
  );
}

function AreaContextMenu({
  onContextUpload,
  onContextRefresh,
}: {
  onContextUpload?: () => void;
  onContextRefresh?: () => void;
}) {
  return (
    <ContextMenuContent>
      <ContextMenuItem disabled={!onContextUpload} onSelect={() => onContextUpload?.()}>
        <Upload className="h-4 w-4" />
        Upload
        <ContextMenuShortcut>U</ContextMenuShortcut>
      </ContextMenuItem>
      {onContextRefresh ? (
        <ContextMenuItem onSelect={() => onContextRefresh()}>
          <RefreshCw className="h-4 w-4" />
          Refresh
          <ContextMenuShortcut>F5</ContextMenuShortcut>
        </ContextMenuItem>
      ) : null}
    </ContextMenuContent>
  );
}

export function FileTable({
  objects,
  selectedKeys,
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
}: FileTableProps) {
  const [sorting, setSorting] = useState<SortingState>([{ id: "name", desc: false }]);

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
        cell: ({ row }) => (
          <div onClick={(event) => event.stopPropagation()}>
            <Checkbox
              checked={selectedKeys.has(row.original.key)}
              onCheckedChange={(value) => onToggleKey(row.original.key, !!value)}
              aria-label={`Select ${row.original.name}`}
            />
          </div>
        ),
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
    [onToggleAll, onToggleKey, selectedKeys]
  );

  const table = useReactTable({
    data: objects,
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
          <div className="flex h-full min-h-[12rem] items-center justify-center p-8 text-sm text-muted-foreground">
            This folder is empty.
          </div>
        </ContextMenuTrigger>
        <AreaContextMenu onContextUpload={onContextUpload} onContextRefresh={onContextRefresh} />
      </ContextMenu>
    );
  }

  return (
    <ContextMenu>
      <ContextMenuTrigger asChild>
        <div className="min-h-full">
          <Table>
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
                <ContextMenu key={row.id}>
                  <ContextMenuTrigger asChild>
                    <TableRow
                      data-state={selectedKeys.has(row.original.key) ? "selected" : undefined}
                      className="cursor-pointer"
                      onClick={() => {
                        const key = row.original.key;
                        onToggleKey(key, !selectedKeys.has(key));
                        onRowClick?.(row.original);
                      }}
                      onDoubleClick={() => {
                        if (row.original.isFolder) onOpenFolder(row.original.key);
                      }}
                    >
                      {row.getVisibleCells().map((cell) => (
                        <TableCell key={cell.id}>
                          {flexRender(cell.column.columnDef.cell, cell.getContext())}
                        </TableCell>
                      ))}
                    </TableRow>
                  </ContextMenuTrigger>
                  <RowContextMenu
                    object={row.original}
                    selectedKeys={selectedKeys}
                    objects={objects}
                    onContextOpen={onContextOpen}
                    onContextDownload={onContextDownload}
                    onContextRename={onContextRename}
                    onContextCopyPath={onContextCopyPath}
                    onContextDelete={onContextDelete}
                  />
                </ContextMenu>
              ))}
            </TableBody>
          </Table>
          {hasMore ? (
            <div className="flex justify-center border-t p-3">
              {loadingMore ? (
                <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />
              ) : (
                <Button variant="outline" size="sm" onClick={onLoadMore}>
                  Load more
                </Button>
              )}
            </div>
          ) : null}
        </div>
      </ContextMenuTrigger>
      <ContextMenuContent>
        <ContextMenuItem disabled={!onContextUpload} onSelect={() => onContextUpload?.()}>
          <Upload className="h-4 w-4" />
          Upload
          <ContextMenuShortcut>U</ContextMenuShortcut>
        </ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>
  );
}
