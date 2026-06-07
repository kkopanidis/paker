import {
  CopyPlus,
  Download,
  FolderPlus,
  Info,
  MoveRight,
  Pencil,
  RefreshCw,
  Database,
  Ruler,
  Search,
  Trash2,
  Upload,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";

interface BrowserToolbarProps {
  disabled?: boolean;
  busy?: boolean;
  hasSelection?: boolean;
  singleSelection?: boolean;
  onUpload: () => void;
  onDownload: () => void;
  onDelete: () => void;
  onRename: () => void;
  onNewFolder: () => void;
  onRefresh: () => void;
  onCopyTo?: () => void;
  onMoveTo?: () => void;
  onProperties?: () => void;
  onCalculateBucketSize?: () => void;
  onIndexBucket?: () => void;
  onSearchIndex?: () => void;
  indexSearchEnabled?: boolean;
}

function ToolbarButton({
  label,
  icon,
  onClick,
  disabled,
}: {
  label: string;
  icon: React.ReactNode;
  onClick: () => void;
  disabled?: boolean;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button variant="ghost" size="icon" onClick={onClick} disabled={disabled} aria-label={label}>
          {icon}
        </Button>
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  );
}

export function BrowserToolbar({
  disabled,
  busy,
  hasSelection,
  singleSelection,
  onUpload,
  onDownload,
  onDelete,
  onRename,
  onNewFolder,
  onRefresh,
  onCopyTo,
  onMoveTo,
  onProperties,
  onCalculateBucketSize,
  onIndexBucket,
  onSearchIndex,
  indexSearchEnabled,
}: BrowserToolbarProps) {
  return (
    <TooltipProvider>
      <div className="flex items-center gap-1 border-b px-2 py-1">
        <ToolbarButton
          label="Upload"
          icon={<Upload className="h-4 w-4" />}
          onClick={onUpload}
          disabled={disabled || busy}
        />
        <ToolbarButton
          label="Download"
          icon={<Download className="h-4 w-4" />}
          onClick={onDownload}
          disabled={disabled || busy || !hasSelection}
        />
        <ToolbarButton
          label="Delete"
          icon={<Trash2 className="h-4 w-4" />}
          onClick={onDelete}
          disabled={disabled || busy || !hasSelection}
        />
        <ToolbarButton
          label="Rename"
          icon={<Pencil className="h-4 w-4" />}
          onClick={onRename}
          disabled={disabled || busy || !singleSelection}
        />
        {(onCopyTo || onMoveTo) && (
          <>
            <Separator orientation="vertical" className="mx-1 h-6" />
            {onCopyTo && (
              <ToolbarButton
                label="Copy to…"
                icon={<CopyPlus className="h-4 w-4" />}
                onClick={onCopyTo}
                disabled={disabled || busy || !hasSelection}
              />
            )}
            {onMoveTo && (
              <ToolbarButton
                label="Move to…"
                icon={<MoveRight className="h-4 w-4" />}
                onClick={onMoveTo}
                disabled={disabled || busy || !hasSelection}
              />
            )}
          </>
        )}
        <Separator orientation="vertical" className="mx-1 h-6" />
        <ToolbarButton
          label="New folder"
          icon={<FolderPlus className="h-4 w-4" />}
          onClick={onNewFolder}
          disabled={disabled || busy}
        />
        <ToolbarButton
          label="Refresh"
          icon={<RefreshCw className={`h-4 w-4 ${busy ? "animate-spin" : ""}`} />}
          onClick={onRefresh}
          disabled={disabled}
        />
        {(onProperties || onCalculateBucketSize) && (
          <>
            <Separator orientation="vertical" className="mx-1 h-6" />
            {onProperties && (
              <ToolbarButton
                label="Bucket properties"
                icon={<Info className="h-4 w-4" />}
                onClick={onProperties}
                disabled={disabled}
              />
            )}
            {onCalculateBucketSize && (
              <ToolbarButton
                label="Calculate bucket size"
                icon={<Ruler className="h-4 w-4" />}
                onClick={onCalculateBucketSize}
                disabled={disabled || busy}
              />
            )}
            {onIndexBucket && (
              <ToolbarButton
                label="Index bucket"
                icon={<Database className="h-4 w-4" />}
                onClick={onIndexBucket}
                disabled={disabled}
              />
            )}
            {onSearchIndex && (
              <ToolbarButton
                label="Search bucket index"
                icon={<Search className="h-4 w-4" />}
                onClick={onSearchIndex}
                disabled={disabled || !indexSearchEnabled}
              />
            )}
          </>
        )}
      </div>
    </TooltipProvider>
  );
}
