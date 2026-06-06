import { useMemo, useRef, useState } from "react";
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
  ChevronRight,
  File,
  Folder,
  FolderOpen,
  FolderSearch,
  Home,
  RefreshCw,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
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
import type { LocalEntry } from "@/types/local";
import { useLocalBrowser } from "@/hooks/useLocalBrowser";

interface LocalFilePanelProps {
  connectionId: string | null;
  onDownloadFromS3?: (keys: string[], destDir: string) => void;
  className?: string;
}

function SortIcon({ sorted }: { sorted: false | "asc" | "desc" }) {
  if (sorted === "asc") return <ArrowUp className="ml-1 inline h-3.5 w-3.5" />;
  if (sorted === "desc") return <ArrowDown className="ml-1 inline h-3.5 w-3.5" />;
  return <ArrowUpDown className="ml-1 inline h-3.5 w-3.5 opacity-40" />;
}

interface LocalTableProps {
  entries: LocalEntry[];
  selectedPaths: Set<string>;
  loading?: boolean;
  onTogglePath: (path: string, selected: boolean) => void;
  onToggleAll: (selected: boolean) => void;
  onNavigateInto: (entry: LocalEntry) => void;
  onContextOpenFolder?: (entry: LocalEntry) => void;
  onContextGoUp?: () => void;
  onContextPickFolder?: () => void;
  onContextRefresh?: () => void;
  onDragStart?: (e: React.DragEvent, entry: LocalEntry) => void;
  onDrop?: (e: React.DragEvent) => void;
  onDragOver?: (e: React.DragEvent) => void;
}

function LocalTable({
  entries,
  selectedPaths,
  loading,
  onTogglePath,
  onToggleAll,
  onNavigateInto,
  onContextOpenFolder,
  onContextGoUp,
  onContextPickFolder,
  onContextRefresh,
  onDragStart,
  onDrop,
  onDragOver,
}: LocalTableProps) {
  const [sorting, setSorting] = useState<SortingState>([{ id: "name", desc: false }]);

  const columns = useMemo<ColumnDef<LocalEntry>[]>(
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
          <div onClick={(e) => e.stopPropagation()}>
            <Checkbox
              checked={selectedPaths.has(row.original.path)}
              onCheckedChange={(value) => onTogglePath(row.original.path, !!value)}
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
          const entry = row.original;
          return (
            <div className="flex items-center gap-2">
              {entry.isDir ? (
                <Folder className="h-4 w-4 shrink-0 text-amber-500" />
              ) : (
                <File className="h-4 w-4 shrink-0 text-muted-foreground" />
              )}
              <span className="truncate">{entry.name}</span>
            </div>
          );
        },
      },
      {
        accessorKey: "size",
        header: "Size",
        cell: ({ row }) => (row.original.isDir ? "—" : formatBytes(row.original.size)),
        sortingFn: (a, b) => a.original.size - b.original.size,
      },
      {
        accessorKey: "modified",
        header: "Modified",
        cell: ({ row }) => formatDate(row.original.modified),
      },
    ],
    [onToggleAll, onTogglePath, selectedPaths]
  );

  const table = useReactTable({
    data: entries,
    columns,
    state: { sorting },
    onSortingChange: setSorting,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
  });

  const areaContextContent = (
    <ContextMenuContent>
      <ContextMenuItem onSelect={() => onContextGoUp?.()}>
        <ArrowUp className="h-4 w-4" />
        Go up
      </ContextMenuItem>
      <ContextMenuItem onSelect={() => onContextPickFolder?.()}>
        <FolderSearch className="h-4 w-4" />
        Pick folder
      </ContextMenuItem>
      {onContextRefresh ? (
        <>
          <ContextMenuSeparator />
          <ContextMenuItem onSelect={() => onContextRefresh()}>
            <RefreshCw className="h-4 w-4" />
            Refresh
          </ContextMenuItem>
        </>
      ) : null}
    </ContextMenuContent>
  );

  if (loading) {
    return (
      <div className="space-y-2 p-4">
        {Array.from({ length: 8 }).map((_, i) => (
          <Skeleton key={i} className="h-9 w-full" />
        ))}
      </div>
    );
  }

  if (entries.length === 0) {
    return (
      <ContextMenu>
        <ContextMenuTrigger asChild>
          <div
            className="flex h-full min-h-[12rem] items-center justify-center p-8 text-sm text-muted-foreground"
            onDragOver={onDragOver}
            onDrop={onDrop}
          >
            This folder is empty.
          </div>
        </ContextMenuTrigger>
        {areaContextContent}
      </ContextMenu>
    );
  }

  return (
    <ContextMenu>
      <ContextMenuTrigger asChild>
        <div className="min-h-full" onDragOver={onDragOver} onDrop={onDrop}>
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
                      data-state={selectedPaths.has(row.original.path) ? "selected" : undefined}
                      className="cursor-pointer"
                      draggable
                      onDragStart={(e) => onDragStart?.(e, row.original)}
                      onClick={() => {
                        const path = row.original.path;
                        onTogglePath(path, !selectedPaths.has(path));
                      }}
                      onDoubleClick={() => {
                        if (row.original.isDir) onNavigateInto(row.original);
                      }}
                    >
                      {row.getVisibleCells().map((cell) => (
                        <TableCell key={cell.id}>
                          {flexRender(cell.column.columnDef.cell, cell.getContext())}
                        </TableCell>
                      ))}
                    </TableRow>
                  </ContextMenuTrigger>
                  <ContextMenuContent>
                    {row.original.isDir ? (
                      <ContextMenuItem onSelect={() => onContextOpenFolder?.(row.original)}>
                        <FolderOpen className="h-4 w-4" />
                        Open folder
                      </ContextMenuItem>
                    ) : null}
                    <ContextMenuItem onSelect={() => onContextGoUp?.()}>
                      <ArrowUp className="h-4 w-4" />
                      Go up
                    </ContextMenuItem>
                    <ContextMenuItem onSelect={() => onContextPickFolder?.()}>
                      <FolderSearch className="h-4 w-4" />
                      Pick folder
                    </ContextMenuItem>
                    <ContextMenuSeparator />
                    <ContextMenuItem onSelect={() => onContextRefresh?.()}>
                      <RefreshCw className="h-4 w-4" />
                      Refresh
                    </ContextMenuItem>
                  </ContextMenuContent>
                </ContextMenu>
              ))}
            </TableBody>
          </Table>
        </div>
      </ContextMenuTrigger>
      {areaContextContent}
    </ContextMenu>
  );
}

export function LocalFilePanel({
  connectionId,
  onDownloadFromS3,
  className,
}: LocalFilePanelProps) {
  const browser = useLocalBrowser(connectionId);
  const {
    cwd,
    entries,
    selectedPaths,
    loading,
    busy,
    breadcrumbs,
    navigateUp,
    navigateInto,
    selectPaths,
    selectAll,
    loadDir,
    pickFolder,
    refresh,
  } = browser;

  const isDragOver = useRef(false);
  const [dropHighlight, setDropHighlight] = useState(false);

  function handleDragStart(e: React.DragEvent, entry: LocalEntry) {
    const paths = selectedPaths.has(entry.path)
      ? Array.from(selectedPaths)
      : [entry.path];
    e.dataTransfer.setData("application/paker-local-paths", JSON.stringify(paths));
    e.dataTransfer.effectAllowed = "copy";
  }

  function handleDragOver(e: React.DragEvent) {
    if (e.dataTransfer.types.includes("application/paker-s3-keys")) {
      e.preventDefault();
      e.dataTransfer.dropEffect = "copy";
      if (!isDragOver.current) {
        isDragOver.current = true;
        setDropHighlight(true);
      }
    }
  }

  function handleDragLeave() {
    isDragOver.current = false;
    setDropHighlight(false);
  }

  function handleDrop(e: React.DragEvent) {
    isDragOver.current = false;
    setDropHighlight(false);
    const raw = e.dataTransfer.getData("application/paker-s3-keys");
    if (!raw) return;
    e.preventDefault();
    try {
      const keys = JSON.parse(raw) as string[];
      if (Array.isArray(keys) && keys.length > 0 && cwd) {
        onDownloadFromS3?.(keys, cwd);
      }
    } catch {
      // invalid drag data
    }
  }

  function handleTogglePath(path: string, selected: boolean) {
    selectPaths([path], selected);
  }

  function handleToggleAll(selected: boolean) {
    if (selected) selectAll();
    else selectPaths(Array.from(selectedPaths), false);
  }

  const rootLabel = breadcrumbs[0]?.label ?? "/";

  return (
    <div
      className={cn("flex h-full flex-col overflow-hidden", className)}
      onDragLeave={handleDragLeave}
    >
      {/* Header */}
      <div className="flex items-center gap-1 border-b px-3 py-2">
        <span className="mr-1 text-sm font-semibold text-foreground">Local</span>
        <div className="flex-1" />
        <Button
          variant="ghost"
          size="icon"
          className="h-7 w-7"
          disabled={busy || loading}
          title="Pick folder"
          onClick={pickFolder}
        >
          <FolderSearch className="h-4 w-4" />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          className="h-7 w-7"
          disabled={loading}
          title="Refresh"
          onClick={refresh}
        >
          <RefreshCw className={cn("h-4 w-4", loading && "animate-spin")} />
        </Button>
      </div>

      {/* Breadcrumb */}
      <nav className="flex flex-wrap items-center gap-1 border-b px-3 py-1.5 text-sm">
        <Button
          variant="ghost"
          size="sm"
          className="h-7 px-2"
          onClick={() => {
            const root = breadcrumbs[0]?.path;
            if (root) void loadDir(root);
            else void navigateUp();
          }}
        >
          <Home className="h-3.5 w-3.5" />
          <span className="font-medium">{rootLabel}</span>
        </Button>
        {breadcrumbs.slice(1).map((segment) => (
          <div key={segment.path} className="flex items-center gap-1">
            <ChevronRight className="h-3.5 w-3.5 text-muted-foreground" />
            <Button
              variant="ghost"
              size="sm"
              className="h-7 px-2 font-normal"
              onClick={() => void loadDir(segment.path)}
            >
              {segment.label}
            </Button>
          </div>
        ))}
      </nav>

      {/* Table area */}
      <div
        className={cn(
          "flex-1 overflow-auto transition-colors",
          dropHighlight && "bg-primary/5 ring-2 ring-inset ring-primary/30"
        )}
      >
        <LocalTable
          entries={entries}
          selectedPaths={selectedPaths}
          loading={loading}
          onTogglePath={handleTogglePath}
          onToggleAll={handleToggleAll}
          onNavigateInto={(entry) => void navigateInto(entry)}
          onContextOpenFolder={(entry) => void navigateInto(entry)}
          onContextGoUp={() => void navigateUp()}
          onContextPickFolder={() => void pickFolder()}
          onContextRefresh={() => void refresh()}
          onDragStart={handleDragStart}
          onDragOver={handleDragOver}
          onDrop={handleDrop}
        />
      </div>
    </div>
  );
}
