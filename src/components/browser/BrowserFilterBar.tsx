import { forwardRef } from "react";
import { Search } from "lucide-react";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";

export type TypeFilter = "all" | "folders" | "files" | "glacier";

export interface BrowserFilterBarProps {
  filterText: string;
  onFilterChange: (value: string) => void;
  totalCount: number;
  filteredCount: number;
  typeFilter: TypeFilter;
  onTypeFilterChange: (value: TypeFilter) => void;
  prefixJump: string;
  onPrefixJump: (value: string) => void;
  onPrefixJumpSubmit: (value: string) => void;
  objectsStale?: boolean;
  refreshingObjects?: boolean;
}

const TYPE_FILTERS: { value: TypeFilter; label: string }[] = [
  { value: "all", label: "All" },
  { value: "folders", label: "Folders" },
  { value: "files", label: "Files" },
  { value: "glacier", label: "Glacier" },
];

function looksLikePath(value: string): boolean {
  const trimmed = value.trim();
  if (!trimmed) return false;
  if (trimmed.includes("/")) return true;
  return /^s3:/i.test(trimmed);
}

export const BrowserFilterBar = forwardRef<HTMLInputElement, BrowserFilterBarProps>(
  function BrowserFilterBar(
    {
      filterText,
      onFilterChange,
      totalCount,
      filteredCount,
      typeFilter,
      onTypeFilterChange,
      prefixJump,
      onPrefixJump,
      onPrefixJumpSubmit,
      objectsStale,
      refreshingObjects,
    },
    ref
  ) {
    const showSyncStatus = objectsStale || refreshingObjects;
    const isFiltered =
      filterText.trim().length > 0 || typeFilter !== "all" || prefixJump.trim().length > 0;
    const displayValue = prefixJump || filterText;

    const handleChange = (value: string) => {
      if (looksLikePath(value)) {
        onPrefixJump(value);
      } else {
        onPrefixJump("");
        onFilterChange(value);
      }
    };

    const handleKeyDown = (event: React.KeyboardEvent<HTMLInputElement>) => {
      if (event.key !== "Enter") return;
      const value = displayValue.trim();
      if (!value) return;
      if (looksLikePath(value)) {
        event.preventDefault();
        onPrefixJumpSubmit(value);
      }
    };

    return (
      <div className="flex flex-col gap-2 border-b px-3 py-2">
        <div className="flex items-center gap-2">
          <div className="relative min-w-0 flex-1">
            <Search className="pointer-events-none absolute left-2.5 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
            <Input
              ref={ref}
              type="search"
              value={displayValue}
              onChange={(e) => handleChange(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder="Filter or jump to prefix…"
              className="pl-8"
              aria-label="Filter or jump to prefix"
            />
          </div>
          <div className="flex shrink-0 items-center gap-2">
            {showSyncStatus && (
              <span className="rounded-md bg-muted px-1.5 py-0.5 text-[10px] font-medium text-muted-foreground">
                {refreshingObjects ? "Syncing…" : "Cached"}
              </span>
            )}
            <span className="text-xs text-muted-foreground tabular-nums">
              {isFiltered
                ? `${filteredCount} of ${totalCount} (filtered)`
                : `${filteredCount} shown`}
            </span>
          </div>
        </div>
        <div className="flex flex-wrap items-center gap-1">
          {TYPE_FILTERS.map(({ value, label }) => (
            <button
              key={value}
              type="button"
              onClick={() => onTypeFilterChange(value)}
              className={cn(
                "rounded-md px-2.5 py-1 text-xs font-medium transition-colors",
                typeFilter === value
                  ? "bg-primary text-primary-foreground"
                  : "bg-muted text-muted-foreground hover:bg-muted/80 hover:text-foreground"
              )}
            >
              {label}
            </button>
          ))}
        </div>
      </div>
    );
  }
);
