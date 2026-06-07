import { Database, Loader2, RefreshCw } from "lucide-react";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Skeleton } from "@/components/ui/skeleton";
import { cn } from "@/lib/utils";
import { CollapsiblePanelHeader } from "@/components/layout/CollapsiblePanelHeader";
import type { BucketInfo } from "@/types/s3";

interface BucketSidebarProps {
  buckets: BucketInfo[];
  selectedBucket: string | null;
  loading: boolean;
  disabled?: boolean;
  collapsed?: boolean;
  onToggleCollapse?: () => void;
  onSelect: (bucket: string) => void;
  onRefresh?: () => void;
}

export function BucketSidebar({
  buckets,
  selectedBucket,
  loading,
  disabled,
  collapsed = false,
  onToggleCollapse,
  onSelect,
  onRefresh,
}: BucketSidebarProps) {
  return (
    <div className="flex h-full flex-col border-r bg-card">
      <CollapsiblePanelHeader
        title="Buckets"
        collapsed={collapsed}
        onToggleCollapse={() => onToggleCollapse?.()}
      />

      {!collapsed && (
      <ScrollArea className="flex-1">
        <div className="space-y-1 p-2">
          {loading &&
            Array.from({ length: 4 }).map((_, i) => (
              <Skeleton key={i} className="h-9 w-full rounded-md" />
            ))}

          {!loading && disabled && (
            <p className="px-2 py-6 text-center text-sm text-muted-foreground">
              Select a connection to view buckets.
            </p>
          )}

          {!loading && !disabled && buckets.length === 0 && (
            <p className="px-2 py-6 text-center text-sm text-muted-foreground">No buckets found.</p>
          )}

          {buckets.map((bucket) => (
            <ContextMenu key={bucket.name}>
              <ContextMenuTrigger asChild>
                <button
                  type="button"
                  disabled={disabled}
                  onClick={() => onSelect(bucket.name)}
                  className={cn(
                    "flex w-full items-center gap-2 rounded-md px-2 py-2 text-left text-sm transition-colors",
                    selectedBucket === bucket.name
                      ? "bg-accent text-accent-foreground"
                      : "hover:bg-muted/60",
                    disabled && "pointer-events-none opacity-50"
                  )}
                >
                  <Database className="h-4 w-4 shrink-0 text-muted-foreground" />
                  <span className="truncate font-medium">{bucket.name}</span>
                </button>
              </ContextMenuTrigger>
              <ContextMenuContent>
                <ContextMenuItem onSelect={() => onSelect(bucket.name)}>Select</ContextMenuItem>
                <ContextMenuItem disabled={!onRefresh} onSelect={() => onRefresh?.()}>
                  <RefreshCw className="h-4 w-4" />
                  Refresh buckets list
                </ContextMenuItem>
              </ContextMenuContent>
            </ContextMenu>
          ))}

          {loading && (
            <div className="flex items-center justify-center py-4 text-muted-foreground">
              <Loader2 className="h-4 w-4 animate-spin" />
            </div>
          )}
        </div>
      </ScrollArea>
      )}
    </div>
  );
}
