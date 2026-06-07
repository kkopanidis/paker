import { useEffect, useState } from "react";
import { Loader2, Ruler } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Skeleton } from "@/components/ui/skeleton";
import { formatIpcError, getBucketMetadata } from "@/lib/tauri";
import { formatBytes, formatDate } from "@/lib/utils";
import type { BucketMetadata, PrefixSizeResult } from "@/types/s3";

interface BucketPropertiesDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  connectionId: string | null;
  bucket: string | null;
  onCalculateSize: () => void;
  sizeResult: PrefixSizeResult | null;
  sizeCalculating: boolean;
}

function DetailRow({ label, value }: { label: string; value?: string | null }) {
  return (
    <div className="grid grid-cols-[8rem_1fr] gap-2 text-xs">
      <dt className="text-muted-foreground">{label}</dt>
      <dd className="break-all">{value || "—"}</dd>
    </div>
  );
}

export function BucketPropertiesDialog({
  open,
  onOpenChange,
  connectionId,
  bucket,
  onCalculateSize,
  sizeResult,
  sizeCalculating,
}: BucketPropertiesDialogProps) {
  const [metadata, setMetadata] = useState<BucketMetadata | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open || !connectionId || !bucket) {
      setMetadata(null);
      setError(null);
      setLoading(false);
      return;
    }

    let cancelled = false;
    setLoading(true);
    setError(null);
    setMetadata(null);

    void getBucketMetadata(connectionId, bucket)
      .then((data) => {
        if (!cancelled) setMetadata(data);
      })
      .catch((err) => {
        if (!cancelled) {
          setError(formatIpcError(err));
        }
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [open, connectionId, bucket]);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle>Bucket properties</DialogTitle>
        </DialogHeader>

        {loading && (
          <div className="space-y-3">
            {Array.from({ length: 6 }).map((_, i) => (
              <Skeleton key={i} className="h-4 w-full" />
            ))}
          </div>
        )}

        {error && <p className="text-sm text-destructive">{error}</p>}

        {!loading && metadata && (
          <dl className="space-y-2.5">
            <DetailRow label="Name" value={metadata.name} />
            <DetailRow label="Connection" value={metadata.connectionName} />
            <DetailRow label="Endpoint" value={metadata.endpoint} />
            <DetailRow label="Region" value={metadata.region} />
            <DetailRow label="Location" value={metadata.location} />
            <DetailRow label="Path style" value={metadata.forcePathStyle ? "Yes" : "No"} />
            <DetailRow label="Created" value={formatDate(metadata.creationDate)} />
            <DetailRow label="Versioning" value={metadata.versioning} />
            {sizeResult && (
              <>
                <DetailRow
                  label="Objects"
                  value={sizeResult.objectCount.toLocaleString()}
                />
                <DetailRow label="Total size" value={formatBytes(sizeResult.totalBytes)} />
              </>
            )}
          </dl>
        )}

        <DialogFooter className="gap-2 sm:justify-between">
          <Button
            variant="outline"
            onClick={onCalculateSize}
            disabled={!connectionId || !bucket || sizeCalculating}
          >
            {sizeCalculating ? (
              <Loader2 className="h-4 w-4 animate-spin" />
            ) : (
              <Ruler className="h-4 w-4" />
            )}
            Calculate total size
          </Button>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Close
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
