import { ArrowUp, ChevronRight, Home, Star, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { cn } from "@/lib/utils";

export interface Bookmark {
  id: string;
  label: string;
  bucket: string;
  prefix: string;
}

interface BreadcrumbProps {
  bucket: string | null;
  segments: { label: string; path: string }[];
  onNavigate: (prefix: string) => void;
  onNavigateUp?: () => void;
  canNavigateUp?: boolean;
  className?: string;
  bookmarks?: Bookmark[];
  onBookmarkNavigate?: (bucket: string, prefix: string) => void;
  onAddBookmark?: () => void;
  onRemoveBookmark?: (id: string) => void;
  currentPrefix?: string;
}

export function Breadcrumb({
  bucket,
  segments,
  onNavigate,
  onNavigateUp,
  canNavigateUp = false,
  className,
  bookmarks = [],
  onBookmarkNavigate,
  onAddBookmark,
  onRemoveBookmark,
  currentPrefix = "",
}: BreadcrumbProps) {
  if (!bucket) {
    return (
      <div className={cn("flex items-center gap-1 px-3 py-2 text-sm text-muted-foreground", className)}>
        Select a bucket to browse files
      </div>
    );
  }

  const isCurrentBookmarked = bookmarks.some(
    (bookmark) => bookmark.bucket === bucket && bookmark.prefix === currentPrefix
  );

  return (
    <nav className={cn("flex flex-wrap items-center gap-1 px-3 py-2 text-sm", className)}>
      {onNavigateUp && (
        <Button
          variant="ghost"
          size="icon"
          className="h-7 w-7 shrink-0"
          onClick={onNavigateUp}
          disabled={!canNavigateUp}
          aria-label="Go up"
        >
          <ArrowUp className="h-3.5 w-3.5" />
        </Button>
      )}
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

      <div className="ml-auto flex items-center gap-0.5">
        {onAddBookmark && (
          <Button
            variant="ghost"
            size="icon"
            className="h-7 w-7"
            onClick={onAddBookmark}
            disabled={isCurrentBookmarked}
            aria-label={isCurrentBookmarked ? "Location bookmarked" : "Bookmark this location"}
            title={isCurrentBookmarked ? "Already bookmarked" : "Bookmark this location"}
          >
            <Star
              className={cn(
                "h-3.5 w-3.5",
                isCurrentBookmarked && "fill-amber-400 text-amber-400"
              )}
            />
          </Button>
        )}

        {bookmarks.length > 0 && onBookmarkNavigate && (
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button
                variant="ghost"
                size="sm"
                className="h-7 gap-1 px-2 text-xs text-muted-foreground"
                aria-label="Jump to bookmark"
              >
                <Star className="h-3 w-3 fill-amber-400 text-amber-400" />
                {bookmarks.length}
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" className="w-64">
              <DropdownMenuLabel>Bookmarks</DropdownMenuLabel>
              <DropdownMenuSeparator />
              {bookmarks.map((bookmark) => {
                const isActive =
                  bookmark.bucket === bucket && bookmark.prefix === currentPrefix;
                const pathLabel = bookmark.prefix
                  ? `${bookmark.bucket}/${bookmark.prefix}`
                  : bookmark.bucket;

                return (
                  <DropdownMenuItem
                    key={bookmark.id}
                    className="group flex items-center justify-between gap-2"
                    onClick={() => onBookmarkNavigate(bookmark.bucket, bookmark.prefix)}
                  >
                    <div className="min-w-0 flex-1">
                      <p
                        className={cn(
                          "truncate text-sm",
                          isActive && "font-medium text-primary"
                        )}
                      >
                        {bookmark.label}
                      </p>
                      <p className="truncate text-xs text-muted-foreground">{pathLabel}</p>
                    </div>
                    {onRemoveBookmark && (
                      <button
                        type="button"
                        className="shrink-0 rounded p-0.5 opacity-0 hover:bg-destructive/10 hover:text-destructive group-hover:opacity-100"
                        aria-label={`Remove bookmark ${bookmark.label}`}
                        onClick={(event) => {
                          event.stopPropagation();
                          onRemoveBookmark(bookmark.id);
                        }}
                      >
                        <Trash2 className="h-3 w-3" />
                      </button>
                    )}
                  </DropdownMenuItem>
                );
              })}
            </DropdownMenuContent>
          </DropdownMenu>
        )}
      </div>
    </nav>
  );
}
