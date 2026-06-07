import { ChevronLeft, ChevronRight } from "lucide-react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

interface CollapsiblePanelHeaderProps {
  title: string;
  collapsed: boolean;
  onToggleCollapse: () => void;
  actions?: React.ReactNode;
  className?: string;
}

export function CollapsiblePanelHeader({
  title,
  collapsed,
  onToggleCollapse,
  actions,
  className,
}: CollapsiblePanelHeaderProps) {
  if (collapsed) {
    return (
      <div
        className={cn(
          "flex h-full flex-col items-center border-r bg-card py-2",
          className
        )}
      >
        <Button
          variant="ghost"
          size="icon"
          className="h-7 w-7 shrink-0"
          onClick={onToggleCollapse}
          aria-label={`Expand ${title}`}
          title={`Expand ${title}`}
        >
          <ChevronRight className="h-4 w-4" />
        </Button>
      </div>
    );
  }

  return (
    <div className={cn("flex items-center justify-between border-b px-3 py-2", className)}>
      <h2 className="text-sm font-semibold">{title}</h2>
      <div className="flex items-center gap-1">
        {actions}
        <Button
          variant="ghost"
          size="icon"
          className="h-7 w-7"
          onClick={onToggleCollapse}
          aria-label={`Collapse ${title}`}
          title={`Collapse ${title}`}
        >
          <ChevronLeft className="h-4 w-4" />
        </Button>
      </div>
    </div>
  );
}
