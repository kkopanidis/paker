import { useState } from "react";
import { Loader2, MoreHorizontal, PlugZap, Plus, Trash2, Pencil, TriangleAlert } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Skeleton } from "@/components/ui/skeleton";
import { cn } from "@/lib/utils";
import type { S3Connection, S3ConnectionInput } from "@/types/connection";
import { CollapsiblePanelHeader } from "@/components/layout/CollapsiblePanelHeader";
import { ConnectionForm } from "./ConnectionForm";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";

interface ConnectionListProps {
  connections: S3Connection[];
  selectedId: string | null;
  loading: boolean;
  testingId: string | null;
  collapsed?: boolean;
  onToggleCollapse?: () => void;
  onSelect: (id: string) => void;
  onSave: (input: S3ConnectionInput, id?: string) => Promise<unknown>;
  onDelete: (id: string) => Promise<unknown>;
  onTest: (id: string) => Promise<unknown>;
}

export function ConnectionList({
  connections,
  selectedId,
  loading,
  testingId,
  collapsed = false,
  onToggleCollapse,
  onSelect,
  onSave,
  onDelete,
  onTest,
}: ConnectionListProps) {
  const [formOpen, setFormOpen] = useState(false);
  const [editing, setEditing] = useState<S3Connection | null>(null);

  const openCreate = () => {
    setEditing(null);
    setFormOpen(true);
  };

  const openEdit = (connection: S3Connection) => {
    setEditing(connection);
    setFormOpen(true);
  };

  return (
    <div className="flex h-full flex-col border-r bg-card">
      <CollapsiblePanelHeader
        title="Connections"
        collapsed={collapsed}
        onToggleCollapse={() => onToggleCollapse?.()}
        actions={
          !collapsed ? (
            <Button variant="ghost" size="icon" className="h-7 w-7" onClick={openCreate} aria-label="Add connection">
              <Plus className="h-4 w-4" />
            </Button>
          ) : undefined
        }
      />

      {!collapsed && (
      <ScrollArea className="flex-1">
        <div className="space-y-1 p-2">
          {loading &&
            Array.from({ length: 3 }).map((_, i) => (
              <Skeleton key={i} className="h-12 w-full rounded-md" />
            ))}

          {!loading && connections.length === 0 && (
            <p className="px-2 py-6 text-center text-sm text-muted-foreground">
              No connections yet. Add one to get started.
            </p>
          )}

          {connections.map((connection) => {
            const isSelected = connection.id === selectedId;
            const isTesting = testingId === connection.id;

            return (
              <ContextMenu key={connection.id}>
                <ContextMenuTrigger asChild>
                  <div
                    className={cn(
                      "group flex items-center gap-1 rounded-md border px-2 py-2 transition-colors",
                      isSelected ? "border-primary bg-accent" : "border-transparent hover:bg-muted/60"
                    )}
                  >
                    <button
                      type="button"
                      className="min-w-0 flex-1 text-left"
                      onClick={() => onSelect(connection.id)}
                    >
                      <div className="truncate text-sm font-medium">{connection.name}</div>
                      <div className="flex items-center gap-1 truncate text-xs text-muted-foreground">
                        <span className="truncate">
                          {connection.endpoint ?? connection.region}
                        </span>
                        {connection.skipTlsVerify && (
                          <Tooltip>
                            <TooltipTrigger asChild>
                              <TriangleAlert
                                className="h-3 w-3 shrink-0 text-amber-500"
                                aria-label="TLS verification disabled"
                              />
                            </TooltipTrigger>
                            <TooltipContent>TLS certificate verification is disabled</TooltipContent>
                          </Tooltip>
                        )}
                      </div>
                    </button>

                    <Button
                      variant="ghost"
                      size="icon"
                      className="h-8 w-8 shrink-0"
                      disabled={isTesting}
                      onClick={() => void onTest(connection.id)}
                      aria-label="Test connection"
                    >
                      {isTesting ? (
                        <Loader2 className="h-4 w-4 animate-spin" />
                      ) : (
                        <PlugZap className="h-4 w-4" />
                      )}
                    </Button>

                    <DropdownMenu>
                      <DropdownMenuTrigger asChild>
                        <Button variant="ghost" size="icon" className="h-8 w-8 shrink-0">
                          <MoreHorizontal className="h-4 w-4" />
                        </Button>
                      </DropdownMenuTrigger>
                      <DropdownMenuContent align="end">
                        <DropdownMenuItem onClick={() => openEdit(connection)}>
                          <Pencil className="h-4 w-4" />
                          Edit
                        </DropdownMenuItem>
                        <DropdownMenuItem
                          className="text-destructive focus:text-destructive"
                          onClick={() => void onDelete(connection.id)}
                        >
                          <Trash2 className="h-4 w-4" />
                          Delete
                        </DropdownMenuItem>
                      </DropdownMenuContent>
                    </DropdownMenu>
                  </div>
                </ContextMenuTrigger>
                <ContextMenuContent>
                  <ContextMenuItem onSelect={() => openEdit(connection)}>
                    <Pencil className="h-4 w-4" />
                    Edit
                  </ContextMenuItem>
                  <ContextMenuItem
                    disabled={isTesting}
                    onSelect={() => void onTest(connection.id)}
                  >
                    <PlugZap className="h-4 w-4" />
                    Test connection
                  </ContextMenuItem>
                  <ContextMenuSeparator />
                  <ContextMenuItem
                    className="text-destructive focus:text-destructive"
                    onSelect={() => void onDelete(connection.id)}
                  >
                    <Trash2 className="h-4 w-4" />
                    Delete
                  </ContextMenuItem>
                </ContextMenuContent>
              </ContextMenu>
            );
          })}
        </div>
      </ScrollArea>
      )}

      <ConnectionForm
        open={formOpen}
        onOpenChange={setFormOpen}
        connection={editing}
        onSubmit={onSave}
      />
    </div>
  );
}
