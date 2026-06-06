import { Skeleton } from "@/components/ui/skeleton";
import { formatBytes, formatDate } from "@/lib/utils";
import type { ObjectHeadDetails, S3Object } from "@/types/s3";

interface ObjectDetailsProps {
  object: S3Object | null;
  details: ObjectHeadDetails | null;
  loading?: boolean;
}

interface DetailRowProps {
  label: string;
  value?: string | null;
  mono?: boolean;
}

function DetailRow({ label, value, mono }: DetailRowProps) {
  return (
    <div className="grid grid-cols-[7rem_1fr] gap-2 text-xs">
      <dt className="text-muted-foreground">{label}</dt>
      <dd className={mono ? "truncate font-mono" : "break-all"}>{value || "—"}</dd>
    </div>
  );
}

function LoadingSkeleton() {
  return (
    <div className="space-y-3 p-4">
      {Array.from({ length: 6 }).map((_, index) => (
        <div key={index} className="grid grid-cols-[7rem_1fr] gap-2">
          <Skeleton className="h-3 w-16" />
          <Skeleton className="h-3 w-full" />
        </div>
      ))}
    </div>
  );
}

export function ObjectDetails({ object, details, loading }: ObjectDetailsProps) {
  if (!object && !loading) {
    return (
      <div className="flex h-full items-center justify-center p-4 text-sm text-muted-foreground">
        Select an object to view details
      </div>
    );
  }

  if (loading) {
    return (
      <div className="h-full overflow-y-auto border-l bg-muted/20">
        <div className="border-b px-4 py-3">
          <Skeleton className="h-4 w-24" />
        </div>
        <LoadingSkeleton />
      </div>
    );
  }

  const size = details?.contentLength ?? object?.size;
  const lastModified = details?.lastModified ?? object?.lastModified;
  const etag = details?.etag ?? object?.etag;
  const storageClass = details?.storageClass ?? object?.storageClass;
  const metadata = details?.metadata ?? {};
  const metadataEntries = Object.entries(metadata);

  return (
    <div className="h-full overflow-y-auto border-l bg-muted/20">
      <div className="border-b px-4 py-3">
        <h3 className="text-sm font-medium">Object details</h3>
        <p className="mt-0.5 truncate text-xs text-muted-foreground">{object?.name}</p>
      </div>
      <dl className="space-y-2.5 p-4">
        <DetailRow label="Key" value={details?.key ?? object?.key} mono />
        <DetailRow
          label="Size"
          value={size !== undefined ? formatBytes(size) : undefined}
        />
        <DetailRow label="Modified" value={formatDate(lastModified)} />
        <DetailRow label="ETag" value={etag} mono />
        <DetailRow label="Type" value={details?.contentType} />
        <DetailRow label="Storage" value={storageClass} />
        {metadataEntries.length > 0 && (
          <div className="space-y-2 pt-2">
            <dt className="text-xs font-medium text-muted-foreground">Metadata</dt>
            <dd className="space-y-2 rounded-md border bg-background px-3 py-2">
              {metadataEntries.map(([key, value]) => (
                <div key={key} className="grid grid-cols-[1fr_1fr] gap-2 text-xs">
                  <span className="truncate font-mono text-muted-foreground">{key}</span>
                  <span className="break-all font-mono">{value}</span>
                </div>
              ))}
            </dd>
          </div>
        )}
      </dl>
    </div>
  );
}
