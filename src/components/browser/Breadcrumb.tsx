import { ChevronRight, Home } from "lucide-react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

interface BreadcrumbProps {
  bucket: string | null;
  segments: { label: string; path: string }[];
  onNavigate: (prefix: string) => void;
  className?: string;
}

export function Breadcrumb({ bucket, segments, onNavigate, className }: BreadcrumbProps) {
  if (!bucket) {
    return (
      <div className={cn("flex items-center gap-1 px-3 py-2 text-sm text-muted-foreground", className)}>
        Select a bucket to browse files
      </div>
    );
  }

  return (
    <nav className={cn("flex flex-wrap items-center gap-1 px-3 py-2 text-sm", className)}>
      <Button
        variant="ghost"
        size="sm"
        className="h-7 px-2"
        onClick={() => onNavigate("")}
      >
        <Home className="h-3.5 w-3.5" />
        <span className="font-medium">{bucket}</span>
      </Button>

      {segments.map((segment) => (
        <div key={segment.path} className="flex items-center gap-1">
          <ChevronRight className="h-3.5 w-3.5 text-muted-foreground" />
          <Button
            variant="ghost"
            size="sm"
            className="h-7 px-2 font-normal"
            onClick={() => onNavigate(segment.path)}
          >
            {segment.label}
          </Button>
        </div>
      ))}
    </nav>
  );
}
