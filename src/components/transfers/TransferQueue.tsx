import { useState } from "react";
import { ChevronDown, ChevronUp, Copy, Download, Pause, Play, Upload, X } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";
import { ScrollArea } from "@/components/ui/scroll-area";
import { formatBytes, cn } from "@/lib/utils";
import type { TransferProgress } from "@/types/s3";

interface TransferQueueProps {
  transfers: TransferProgress[];
  activeCount: number;
  onClearCompleted: () => void;
  onCancel: (transferId: string) => void;
  onPause: (transferId: string) => void;
  onResume: (transferId: string) => void;
}

function statusVariant(status: TransferProgress["status"]) {
  switch (status) {
    case "completed":
      return "secondary" as const;
    case "failed":
      return "destructive" as const;
    case "in_progress":
      return "default" as const;
    case "cancelled":
      return "outline" as const;
    case "paused":
      return "secondary" as const;
    default:
      return "outline" as const;
  }
}

function directionIcon(direction: TransferProgress["direction"]) {
  if (direction === "upload") return Upload;
  if (direction === "copy") return Copy;
  return Download;
}

function progressPercent(transfer: TransferProgress): number {
  if (!transfer.total || transfer.total === 0) {
    return transfer.status === "completed" ? 100 : 0;
  }
  return Math.min(100, Math.round((transfer.bytes / transfer.total) * 100));
}

export function TransferQueue({
  transfers,
  activeCount,
  onClearCompleted,
  onCancel,
  onPause,
  onResume,
}: TransferQueueProps) {
  const [collapsed, setCollapsed] = useState(false);

  if (transfers.length === 0) return null;

  return (
    <div className="border-t bg-card">
      <div className="flex items-center justify-between px-3 py-2">
        <button
          type="button"
          className="flex items-center gap-2 text-sm font-medium"
          onClick={() => setCollapsed((v) => !v)}
        >
          {collapsed ? <ChevronUp className="h-4 w-4" /> : <ChevronDown className="h-4 w-4" />}
          Transfers
          {activeCount > 0 && (
            <Badge variant="default" className="ml-1">
              {activeCount} active
            </Badge>
          )}
        </button>

        <Button variant="ghost" size="sm" onClick={onClearCompleted}>
          Clear completed
        </Button>
      </div>

      {!collapsed && (
        <ScrollArea className="max-h-48 border-t">
          <div className="space-y-2 p-3">
            {transfers.map((transfer) => {
              const percent = progressPercent(transfer);
              const Icon = directionIcon(transfer.direction);
              const canCancel =
                transfer.status === "started" ||
                transfer.status === "in_progress" ||
                transfer.status === "paused";
              const canPause = transfer.status === "in_progress";
              const canResume = transfer.status === "paused";

              return (
                <div key={transfer.transferId} className="rounded-md border p-2">
                  <div className="mb-2 flex items-center gap-2">
                    <Icon className="h-4 w-4 shrink-0 text-muted-foreground" />
                    <span className="min-w-0 flex-1 truncate text-sm">{transfer.fileName}</span>
                    <Badge variant={statusVariant(transfer.status)}>{transfer.status}</Badge>
                    {canResume && (
                      <Button
                        variant="ghost"
                        size="icon"
                        className="h-6 w-6"
                        onClick={() => onResume(transfer.transferId)}
                        title="Resume"
                      >
                        <Play className="h-3 w-3" />
                      </Button>
                    )}
                    {canPause && (
                      <Button
                        variant="ghost"
                        size="icon"
                        className="h-6 w-6"
                        onClick={() => onPause(transfer.transferId)}
                        title="Pause"
                      >
                        <Pause className="h-3 w-3" />
                      </Button>
                    )}
                    {canCancel && (
                      <Button
                        variant="ghost"
                        size="icon"
                        className="h-6 w-6"
                        onClick={() => onCancel(transfer.transferId)}
                        title="Cancel"
                      >
                        <X className="h-3 w-3" />
                      </Button>
                    )}
                  </div>
                  <Progress
                    value={percent}
                    className={cn(
                      (transfer.status === "failed" || transfer.status === "cancelled") &&
                        "opacity-60"
                    )}
                  />
                  <div className="mt-1 flex justify-between text-xs text-muted-foreground">
                    <span>
                      {formatBytes(transfer.bytes)}
                      {transfer.total ? ` / ${formatBytes(transfer.total)}` : ""}
                    </span>
                    <span>{percent}%</span>
                  </div>
                  {transfer.status === "failed" && (
                    <p className="mt-1 text-xs text-destructive">Transfer failed</p>
                  )}
                  {transfer.status === "cancelled" && (
                    <p className="mt-1 text-xs text-muted-foreground">Transfer cancelled</p>
                  )}
                </div>
              );
            })}
          </div>
        </ScrollArea>
      )}
    </div>
  );
}
