import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Loader2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { calculatePrefixSize, formatIpcError } from "@/lib/tauri";
import { formatBytes } from "@/lib/utils";
import type { PrefixSizeProgress, PrefixSizeResult } from "@/types/s3";

interface SizeCalculationDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  connectionId: string | null;
  bucket: string | null;
  prefix: string;
  title: string;
  onComplete?: (result: PrefixSizeResult) => void;
}

export function SizeCalculationDialog({
  open,
  onOpenChange,
  connectionId,
  bucket,
  prefix,
  title,
  onComplete,
}: SizeCalculationDialogProps) {
  const [running, setRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [progress, setProgress] = useState<PrefixSizeProgress | null>(null);
  const [result, setResult] = useState<PrefixSizeResult | null>(null);

  useEffect(() => {
    if (!open) {
      setRunning(false);
      setError(null);
      setProgress(null);
      setResult(null);
      return;
    }

    if (!connectionId || !bucket) return;

    let cancelled = false;
    let unlisten: (() => void) | undefined;

    async function run() {
      setRunning(true);
      setError(null);
      setProgress(null);
      setResult(null);

      try {
        unlisten = await listen<PrefixSizeProgress>("prefix-size-progress", (event) => {
          if (cancelled) return;
          const payload = event.payload;
          setProgress(payload);
          if (payload.error) {
            setError(payload.error);
          }
        });

        const final = await calculatePrefixSize(connectionId!, bucket!, prefix);
        if (!cancelled) {
          setResult(final);
          onComplete?.(final);
        }
      } catch (err) {
        if (!cancelled) {
          setError(formatIpcError(err));
        }
      } finally {
        if (!cancelled) setRunning(false);
      }
    }

    void run();

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [open, connectionId, bucket, prefix]);

  const objectCount = result?.objectCount ?? progress?.objectCount ?? 0;
  const totalBytes = result?.totalBytes ?? progress?.totalBytes ?? 0;
  const done = !!result || !!error;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-sm">
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
        </DialogHeader>

        <div className="space-y-3 text-sm">
          {running && !done && (
            <div className="flex items-center gap-2 text-muted-foreground">
              <Loader2 className="h-4 w-4 animate-spin" />
              Scanning objects…
            </div>
          )}

          {error && <p className="text-destructive">{error}</p>}

          {(running || done) && !error && (
            <dl className="space-y-2">
              <div className="flex justify-between gap-4">
                <dt className="text-muted-foreground">Objects</dt>
                <dd className="font-mono">{objectCount.toLocaleString()}</dd>
              </div>
              <div className="flex justify-between gap-4">
                <dt className="text-muted-foreground">Total size</dt>
                <dd className="font-mono">{formatBytes(totalBytes)}</dd>
              </div>
            </dl>
          )}

          {done && !error && (
            <p className="text-muted-foreground">Calculation complete.</p>
          )}
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)} disabled={running}>
            {done ? "Close" : "Cancel"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
