import { useEffect, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import {
  Calculator,
  Copy,
  ExternalLink,
  Link,
  Loader2,
  PanelRightClose,
  PanelRightOpen,
  RefreshCw,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { formatBytes, formatDate } from "@/lib/utils";
import type { ObjectHeadDetails, PrefixSizeProgress, PrefixSizeResult, S3Object } from "@/types/s3";

export function buildS3Uri(bucket: string, key: string): string {
  return `s3://${bucket}/${key}`;
}

export function buildHttpsUrl(
  endpoint: string | null,
  bucket: string,
  key: string,
  forcePathStyle?: boolean
): string {
  if (!endpoint) {
    return `https://${bucket}.s3.amazonaws.com/${key}`;
  }

  const base = endpoint.replace(/\/$/, "");
  if (forcePathStyle) {
    return `${base}/${bucket}/${key}`;
  }

  try {
    const url = new URL(base);
    const port = url.port ? `:${url.port}` : "";
    return `${url.protocol}//${bucket}.${url.host}${port}/${key}`;
  } catch {
    return `${base}/${bucket}/${key}`;
  }
}

interface ObjectDetailsProps {
  object: S3Object | null;
  details: ObjectHeadDetails | null;
  loading?: boolean;
  collapsed?: boolean;
  onToggleCollapse?: () => void;
  selectedObjects?: S3Object[];
  bucket?: string | null;
  endpoint?: string | null;
  forcePathStyle?: boolean;
  connectionId?: string | null;
  onCopyS3Uri?: (uri: string) => void;
  onCopyHttpsUrl?: (url: string) => void;
  onCopyPresignedUrl?: () => void;
  presignedLoading?: boolean;
  previewPath?: string | null;
  previewLoading?: boolean;
  onOpenExternally?: () => void;
  folderSize?: PrefixSizeResult | null;
  folderSizeProgress?: PrefixSizeProgress | null;
  folderSizeLoading?: boolean;
  folderSizeError?: string | null;
  onCalculateFolderSize?: () => void;
  detailsFromCache?: boolean;
  onRefreshDetails?: () => void;
}

interface DetailRowProps {
  label: string;
  value?: string | null;
  mono?: boolean;
  action?: React.ReactNode;
}

function DetailRow({ label, value, mono, action }: DetailRowProps) {
  return (
    <div className="grid grid-cols-[7rem_1fr] gap-2 text-xs">
      <dt className="text-muted-foreground">{label}</dt>
      <dd className="flex min-w-0 items-center gap-1.5">
        <span className={mono ? "min-w-0 truncate font-mono" : "min-w-0 break-all"}>
          {value || "—"}
        </span>
        {action}
      </dd>
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

function folderSizeAction({
  loading,
  hasResult,
  disabled,
  onCalculate,
}: {
  loading: boolean;
  hasResult: boolean;
  disabled?: boolean;
  onCalculate?: () => void;
}) {
  if (!onCalculate) return null;

  return (
    <Button
      type="button"
      variant="ghost"
      size="icon"
      className="h-6 w-6 shrink-0"
      disabled={disabled || loading}
      onClick={onCalculate}
      aria-label={hasResult ? "Recalculate folder size" : "Calculate folder size"}
      title={hasResult ? "Recalculate size" : "Calculate size"}
    >
      {loading ? (
        <Loader2 className="h-3.5 w-3.5 animate-spin" />
      ) : hasResult ? (
        <RefreshCw className="h-3.5 w-3.5" />
      ) : (
        <Calculator className="h-3.5 w-3.5" />
      )}
    </Button>
  );
}

function MultiSelectSummary({ objects }: { objects: S3Object[] }) {
  const files = objects.filter((item) => !item.isFolder);
  const folders = objects.filter((item) => item.isFolder);
  const totalBytes = files.reduce((sum, item) => sum + item.size, 0);

  return (
    <dl className="space-y-2.5 p-4">
      <DetailRow label="Selected" value={`${objects.length} items`} />
      <DetailRow
        label="Contents"
        value={`${files.length} file${files.length === 1 ? "" : "s"}, ${folders.length} folder${folders.length === 1 ? "" : "s"}`}
      />
      {files.length > 0 && (
        <DetailRow label="Total size" value={formatBytes(totalBytes)} />
      )}
    </dl>
  );
}

function PreviewSection({
  previewPath,
  previewLoading,
  contentType,
}: {
  previewPath?: string | null;
  previewLoading?: boolean;
  contentType?: string;
}) {
  const [textPreview, setTextPreview] = useState<string | null>(null);
  const [textError, setTextError] = useState<string | null>(null);

  const isImage = contentType?.startsWith("image/") ?? false;
  const isText = contentType?.startsWith("text/") ?? false;
  const showPreview = previewPath && (isImage || isText);

  useEffect(() => {
    if (!previewPath || !isText) {
      setTextPreview(null);
      setTextError(null);
      return;
    }

    let cancelled = false;
    const src = convertFileSrc(previewPath);

    void fetch(src)
      .then((response) => {
        if (!response.ok) throw new Error("Failed to load preview");
        return response.arrayBuffer();
      })
      .then((buffer) => {
        if (cancelled) return;
        const slice = buffer.byteLength > 2048 ? buffer.slice(0, 2048) : buffer;
        const decoder = new TextDecoder("utf-8", { fatal: false });
        setTextPreview(decoder.decode(slice));
        setTextError(null);
      })
      .catch((error: unknown) => {
        if (cancelled) return;
        setTextPreview(null);
        setTextError(error instanceof Error ? error.message : "Failed to load preview");
      });

    return () => {
      cancelled = true;
    };
  }, [previewPath, isText]);

  if (!showPreview && !previewLoading) return null;

  return (
    <div className="border-t px-4 py-3">
      <h4 className="mb-2 text-xs font-medium text-muted-foreground">Preview</h4>
      {previewLoading ? (
        <div className="flex items-center justify-center py-8">
          <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />
        </div>
      ) : isImage && previewPath ? (
        <img
          src={convertFileSrc(previewPath)}
          alt="Object preview"
          className="max-h-48 w-full rounded-md border object-contain bg-background"
        />
      ) : isText ? (
        textError ? (
          <p className="text-xs text-destructive">{textError}</p>
        ) : textPreview !== null ? (
          <pre className="max-h-48 overflow-auto rounded-md border bg-background p-2 text-xs font-mono whitespace-pre-wrap break-all">
            {textPreview}
            {textPreview.length >= 2048 && (
              <span className="text-muted-foreground">…</span>
            )}
          </pre>
        ) : (
          <Skeleton className="h-24 w-full" />
        )
      ) : null}
    </div>
  );
}

function CollapsedPeekBar({
  object,
  onToggleCollapse,
}: {
  object: S3Object | null;
  onToggleCollapse?: () => void;
}) {
  return (
    <button
      type="button"
      className="flex h-full w-8 shrink-0 flex-col items-center gap-2 border-l bg-muted/20 px-1 py-3 hover:bg-muted/40"
      onClick={onToggleCollapse}
      aria-label="Expand details panel"
      title={object?.name ?? "Details"}
    >
      <PanelRightOpen className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
      <span
        className="max-h-full truncate text-[10px] text-muted-foreground [writing-mode:vertical-rl] rotate-180"
        style={{ maxHeight: "calc(100% - 2rem)" }}
      >
        {object?.name ?? "Details"}
      </span>
      {object && !object.isFolder && (
        <span className="text-[9px] text-muted-foreground [writing-mode:vertical-rl] rotate-180">
          {formatBytes(object.size)}
        </span>
      )}
    </button>
  );
}

function DetailsHeader({
  title,
  subtitle,
  collapsed,
  detailsFromCache,
  onRefreshDetails,
  onToggleCollapse,
}: {
  title: string;
  subtitle?: string;
  collapsed?: boolean;
  detailsFromCache?: boolean;
  onRefreshDetails?: () => void;
  onToggleCollapse?: () => void;
}) {
  return (
    <div className="flex items-start gap-1 border-b px-4 py-3">
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <h3 className="text-sm font-medium">{title}</h3>
          {detailsFromCache && (
            <span className="rounded-md bg-muted px-1.5 py-0.5 text-[10px] font-medium text-muted-foreground">
              Cached
            </span>
          )}
        </div>
        {subtitle && (
          <p className="mt-0.5 truncate text-xs text-muted-foreground">{subtitle}</p>
        )}
      </div>
      {onRefreshDetails && (
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className="h-7 w-7 shrink-0"
          onClick={onRefreshDetails}
          aria-label="Refresh details"
          title="Refresh details"
        >
          <RefreshCw className="h-3.5 w-3.5" />
        </Button>
      )}
      {onToggleCollapse && (
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className="h-7 w-7 shrink-0"
          onClick={onToggleCollapse}
          aria-label={collapsed ? "Expand details panel" : "Collapse details panel"}
        >
          {collapsed ? (
            <PanelRightOpen className="h-4 w-4" />
          ) : (
            <PanelRightClose className="h-4 w-4" />
          )}
        </Button>
      )}
    </div>
  );
}

function ActionButtons({
  bucket,
  key,
  endpoint,
  forcePathStyle,
  onCopyS3Uri,
  onCopyHttpsUrl,
  onCopyPresignedUrl,
  presignedLoading,
  onOpenExternally,
}: {
  bucket: string;
  key: string;
  endpoint?: string | null;
  forcePathStyle?: boolean;
  onCopyS3Uri?: (uri: string) => void;
  onCopyHttpsUrl?: (url: string) => void;
  onCopyPresignedUrl?: () => void;
  presignedLoading?: boolean;
  onOpenExternally?: () => void;
}) {
  const s3Uri = buildS3Uri(bucket, key);
  const httpsUrl = buildHttpsUrl(endpoint ?? null, bucket, key, forcePathStyle);

  return (
    <div className="flex flex-wrap gap-1 border-b px-4 py-2">
      {onCopyS3Uri && (
        <Button
          type="button"
          variant="outline"
          size="sm"
          className="h-7 gap-1 px-2 text-xs"
          onClick={() => onCopyS3Uri(s3Uri)}
        >
          <Copy className="h-3 w-3" />
          s3://
        </Button>
      )}
      {onCopyHttpsUrl && (
        <Button
          type="button"
          variant="outline"
          size="sm"
          className="h-7 gap-1 px-2 text-xs"
          onClick={() => onCopyHttpsUrl(httpsUrl)}
        >
          <Link className="h-3 w-3" />
          HTTPS
        </Button>
      )}
      {onCopyPresignedUrl && (
        <Button
          type="button"
          variant="outline"
          size="sm"
          className="h-7 gap-1 px-2 text-xs"
          disabled={presignedLoading}
          onClick={onCopyPresignedUrl}
        >
          {presignedLoading ? (
            <Loader2 className="h-3 w-3 animate-spin" />
          ) : (
            <Copy className="h-3 w-3" />
          )}
          Presigned
        </Button>
      )}
      {onOpenExternally && (
        <Button
          type="button"
          variant="outline"
          size="sm"
          className="h-7 gap-1 px-2 text-xs"
          onClick={onOpenExternally}
        >
          <ExternalLink className="h-3 w-3" />
          Open
        </Button>
      )}
    </div>
  );
}

export function ObjectDetails({
  object,
  details,
  loading,
  collapsed,
  onToggleCollapse,
  selectedObjects = [],
  bucket,
  endpoint,
  forcePathStyle,
  onCopyS3Uri,
  onCopyHttpsUrl,
  onCopyPresignedUrl,
  presignedLoading,
  previewPath,
  previewLoading,
  onOpenExternally,
  folderSize,
  folderSizeProgress,
  folderSizeLoading,
  folderSizeError,
  onCalculateFolderSize,
  detailsFromCache,
  onRefreshDetails,
}: ObjectDetailsProps) {
  const isMultiSelect = selectedObjects.length >= 2;

  if (collapsed) {
    return (
      <CollapsedPeekBar
        object={isMultiSelect ? selectedObjects[0] ?? object : object}
        onToggleCollapse={onToggleCollapse}
      />
    );
  }

  if (!object && !loading && selectedObjects.length === 0) {
    return (
      <div className="flex h-full items-center justify-center border-l bg-muted/20 p-4 text-sm text-muted-foreground">
        Select an object to view details
      </div>
    );
  }

  if (loading) {
    return (
      <div className="h-full overflow-y-auto border-l bg-muted/20">
        <DetailsHeader title="Loading…" onToggleCollapse={onToggleCollapse} />
        <LoadingSkeleton />
      </div>
    );
  }

  if (isMultiSelect) {
    return (
      <div className="h-full overflow-y-auto border-l bg-muted/20">
        <DetailsHeader
          title="Multiple selection"
          subtitle={`${selectedObjects.length} items selected`}
          onToggleCollapse={onToggleCollapse}
        />
        <MultiSelectSummary objects={selectedObjects} />
      </div>
    );
  }

  if (!object) {
    return (
      <div className="flex h-full items-center justify-center border-l bg-muted/20 p-4 text-sm text-muted-foreground">
        Select an object to view details
      </div>
    );
  }

  const isFolder = object.isFolder;
  const size = details?.contentLength ?? object.size;
  const lastModified = details?.lastModified ?? object.lastModified;
  const etag = details?.etag ?? object.etag;
  const storageClass = details?.storageClass ?? object.storageClass;
  const metadata = details?.metadata ?? {};
  const metadataEntries = Object.entries(metadata);
  const contentType = details?.contentType;
  const showActions =
    !isFolder &&
    bucket &&
    (onCopyS3Uri || onCopyHttpsUrl || onCopyPresignedUrl || onOpenExternally);

  const folderSizeLabel = (() => {
    if (folderSizeError) return "Failed";
    if (folderSize) return formatBytes(folderSize.totalBytes);
    if (folderSizeLoading && folderSizeProgress) {
      return formatBytes(folderSizeProgress.totalBytes);
    }
    if (folderSizeLoading) return "Calculating…";
    return undefined;
  })();

  const folderObjectCount =
    folderSize?.objectCount ?? (folderSizeLoading ? folderSizeProgress?.objectCount : undefined);

  return (
    <div className="flex h-full flex-col overflow-hidden border-l bg-muted/20">
      <DetailsHeader
        title={isFolder ? "Folder details" : "Object details"}
        subtitle={object.name}
        detailsFromCache={!isFolder ? detailsFromCache : undefined}
        onRefreshDetails={!isFolder ? onRefreshDetails : undefined}
        onToggleCollapse={onToggleCollapse}
      />

      {showActions && (
        <ActionButtons
          bucket={bucket!}
          key={object.key}
          endpoint={endpoint}
          forcePathStyle={forcePathStyle}
          onCopyS3Uri={onCopyS3Uri}
          onCopyHttpsUrl={onCopyHttpsUrl}
          onCopyPresignedUrl={onCopyPresignedUrl}
          presignedLoading={presignedLoading}
          onOpenExternally={onOpenExternally}
        />
      )}

      <div className="min-h-0 flex-1 overflow-y-auto">
        <dl className="space-y-2.5 p-4">
          <DetailRow label="Key" value={details?.key ?? object.key} mono />
          {isFolder ? (
            <>
              <DetailRow
                label="Size"
                value={folderSizeLabel}
                action={folderSizeAction({
                  loading: !!folderSizeLoading,
                  hasResult: !!folderSize,
                  onCalculate: onCalculateFolderSize,
                })}
              />
              {folderSizeError && (
                <p className="text-xs text-destructive">{folderSizeError}</p>
              )}
              {folderObjectCount !== undefined && (
                <DetailRow label="Objects" value={folderObjectCount.toLocaleString()} />
              )}
            </>
          ) : (
            <DetailRow
              label="Size"
              value={size !== undefined ? formatBytes(size) : undefined}
            />
          )}
          <DetailRow label="Modified" value={formatDate(lastModified)} />
          {!isFolder && (
            <>
              <DetailRow label="ETag" value={etag} mono />
              <DetailRow label="Type" value={contentType} />
              <DetailRow label="Storage" value={storageClass} />
            </>
          )}
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

        <PreviewSection
          previewPath={previewPath}
          previewLoading={previewLoading}
          contentType={contentType}
        />
      </div>
    </div>
  );
}
