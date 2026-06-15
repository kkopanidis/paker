import { useCallback, useEffect, useState } from "react";
import { BarChart3, Loader2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { ScrollArea } from "@/components/ui/scroll-area";
import { assistantGetBucketReport, formatIpcError } from "@/lib/tauri";
import { formatBytes } from "@/lib/utils";
import type { BucketReport } from "@/types/assistant";

interface BucketReportDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  connectionId: string | null;
  bucket: string | null;
}

export function BucketReportDialog({
  open,
  onOpenChange,
  connectionId,
  bucket,
}: BucketReportDialogProps) {
  const [report, setReport] = useState<BucketReport | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const loadReport = useCallback(async () => {
    if (!connectionId || !bucket) return;
    setLoading(true);
    setError(null);
    try {
      const data = await assistantGetBucketReport(connectionId, bucket, 10);
      setReport(data);
    } catch (err) {
      setReport(null);
      setError(formatIpcError(err));
    } finally {
      setLoading(false);
    }
  }, [connectionId, bucket]);

  useEffect(() => {
    if (open) {
      void loadReport();
    } else {
      setReport(null);
      setError(null);
    }
  }, [open, loadReport]);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <BarChart3 className="h-4 w-4" />
            Bucket analysis
          </DialogTitle>
        </DialogHeader>

        {loading && (
          <div className="flex items-center gap-2 py-6 text-sm text-muted-foreground">
            <Loader2 className="h-4 w-4 animate-spin" />
            Analyzing index…
          </div>
        )}

        {error && !loading && (
          <p className="text-sm text-destructive">{error}</p>
        )}

        {report && !loading && (
          <div className="space-y-4 text-sm">
            <dl className="grid grid-cols-2 gap-x-4 gap-y-2">
              <dt className="text-muted-foreground">Objects</dt>
              <dd className="tabular-nums">{report.totalObjects.toLocaleString()}</dd>
              <dt className="text-muted-foreground">Total size</dt>
              <dd className="tabular-nums">{formatBytes(report.totalBytes)}</dd>
              <dt className="text-muted-foreground">Glacier-class</dt>
              <dd className="tabular-nums">
                {report.glacierObjectCount.toLocaleString()} (
                {formatBytes(report.glacierBytes)})
              </dd>
              <dt className="text-muted-foreground">
                Files &lt; {formatBytes(report.smallFileThresholdBytes)}
              </dt>
              <dd className="tabular-nums">{report.smallFileCount.toLocaleString()}</dd>
            </dl>

            <div>
              <h4 className="mb-2 font-medium">Top prefixes by size</h4>
              <ScrollArea className="h-48 rounded-md border">
                {report.topPrefixesByBytes.length === 0 ? (
                  <p className="p-3 text-muted-foreground">No indexed objects.</p>
                ) : (
                  report.topPrefixesByBytes.map((row) => (
                    <div
                      key={row.prefix}
                      className="flex items-center justify-between gap-2 border-b px-3 py-2 last:border-b-0"
                    >
                      <span className="min-w-0 truncate font-mono text-xs">{row.prefix}</span>
                      <span className="shrink-0 text-xs text-muted-foreground tabular-nums">
                        {formatBytes(row.totalBytes)} · {row.objectCount.toLocaleString()}
                      </span>
                    </div>
                  ))
                )}
              </ScrollArea>
            </div>
          </div>
        )}

        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Close
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
