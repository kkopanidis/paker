import { useEffect, useState } from "react";
import { Loader2 } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { exportBucketIndexCsv } from "@/lib/tauri";
import type { useBucketIndex } from "@/hooks/useBucketIndex";

interface BucketIndexDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  connectionId: string | null;
  bucket: string | null;
  index: ReturnType<typeof useBucketIndex>;
}

function statusLabel(status: string | undefined): string {
  switch (status) {
    case "running":
      return "Indexing…";
    case "paused":
      return "Paused";
    case "completed":
      return "Complete";
    case "stale":
      return "Stale — rebuild recommended";
    case "failed":
      return "Failed";
    case "cancelled":
      return "Cancelled";
    default:
      return "Not indexed";
  }
}

export function BucketIndexDialog({
  open,
  onOpenChange,
  connectionId,
  bucket,
  index,
}: BucketIndexDialogProps) {
  const [exporting, setExporting] = useState(false);

  useEffect(() => {
    if (open) {
      void index.refreshStatus();
    }
  }, [open, index.refreshStatus]);

  const objectCount = index.progress?.objectCount ?? index.meta?.objectCount ?? 0;
  const status = index.progress?.status ?? index.meta?.status ?? "idle";
  const error = index.progress?.error ?? index.meta?.error;
  const running = status === "running";
  const paused = status === "paused";
  const canExport = index.isSearchable && objectCount > 0;

  const handleExport = async () => {
    if (!connectionId || !bucket) return;
    setExporting(true);
    try {
      const path = await exportBucketIndexCsv(connectionId, bucket);
      toast.success("Index exported", { description: path });
    } catch (err) {
      toast.error("Export failed", {
        description: err instanceof Error ? err.message : String(err),
      });
    } finally {
      setExporting(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle>Bucket index{bucket ? `: ${bucket}` : ""}</DialogTitle>
        </DialogHeader>

        <div className="space-y-3 text-sm">
          <p className="text-muted-foreground">
            Builds a local searchable index of all objects in this bucket. Useful for weak
            connections and cross-prefix search. This may use many List API calls on large
            buckets.
          </p>

          {(running || paused) && (
            <div className="flex items-center gap-2 text-muted-foreground">
              <Loader2 className={`h-4 w-4 ${running ? "animate-spin" : ""}`} />
              {statusLabel(status)}
            </div>
          )}

          {error && <p className="text-destructive">{error}</p>}

          <dl className="space-y-2">
            <div className="flex justify-between gap-4">
              <dt className="text-muted-foreground">Status</dt>
              <dd>{statusLabel(status)}</dd>
            </div>
            <div className="flex justify-between gap-4">
              <dt className="text-muted-foreground">Objects indexed</dt>
              <dd className="font-mono tabular-nums">{objectCount.toLocaleString()}</dd>
            </div>
          </dl>
        </div>

        <DialogFooter className="flex-wrap gap-2 sm:justify-between">
          <div className="flex flex-wrap gap-2">
            {!index.isActive && (
              <Button
                onClick={() => void index.start(true)}
                disabled={!connectionId || !bucket}
              >
                {index.meta?.objectCount ? "Rebuild index" : "Start indexing"}
              </Button>
            )}
            {running && (
              <>
                <Button variant="outline" onClick={() => void index.pause()}>
                  Pause
                </Button>
                <Button variant="outline" onClick={() => void index.cancel()}>
                  Cancel
                </Button>
              </>
            )}
            {paused && (
              <>
                <Button onClick={() => void index.resume()}>Resume</Button>
                <Button variant="outline" onClick={() => void index.cancel()}>
                  Cancel
                </Button>
              </>
            )}
            {canExport && (
              <Button variant="outline" onClick={() => void handleExport()} disabled={exporting}>
                {exporting ? "Exporting…" : "Export CSV"}
              </Button>
            )}
          </div>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Close
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
