import {
  Download,
  FolderPlus,
  Pencil,
  RefreshCw,
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
      </div>
    </TooltipProvider>
  );
}
