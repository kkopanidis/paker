import { useCallback, useEffect, useState } from "react";
import { Loader2, Search, Sparkles } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { assistantParseQuery, assistantRunIndexQuery } from "@/lib/tauri";
import { formatBytes } from "@/lib/utils";
import type { ParsedAssistantQuery } from "@/types/assistant";
import type { IndexedObject } from "@/types/s3";

interface BucketIndexSearchDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  connectionId: string | null;
  bucket: string | null;
  indexStale?: boolean;
  onNavigate: (key: string) => void;
}

export function BucketIndexSearchDialog({
  open,
  onOpenChange,
  connectionId,
  bucket,
  indexStale,
  onNavigate,
}: BucketIndexSearchDialogProps) {
  const [query, setQuery] = useState("");
  const [parsed, setParsed] = useState<ParsedAssistantQuery | null>(null);
  const [results, setResults] = useState<IndexedObject[]>([]);
  const [loading, setLoading] = useState(false);
  const [parsing, setParsing] = useState(false);
  const [searched, setSearched] = useState(false);

  useEffect(() => {
    if (!open) {
      setQuery("");
      setParsed(null);
      setResults([]);
      setSearched(false);
    }
  }, [open]);

  useEffect(() => {
    if (!open || !query.trim()) {
      setParsed(null);
      return;
    }

    const handle = window.setTimeout(() => {
      setParsing(true);
      void assistantParseQuery(query.trim())
        .then(setParsed)
        .catch(() => setParsed(null))
        .finally(() => setParsing(false));
    }, 250);

    return () => window.clearTimeout(handle);
  }, [open, query]);

  const runSearch = useCallback(async () => {
    if (!connectionId || !bucket || !query.trim()) return;

    setLoading(true);
    setSearched(true);
    try {
      const interpretation =
        parsed ?? (await assistantParseQuery(query.trim()));
      setParsed(interpretation);
      const hits = await assistantRunIndexQuery(
        connectionId,
        bucket,
        interpretation.query
      );
      setResults(hits);
    } catch {
      setResults([]);
    } finally {
      setLoading(false);
    }
  }, [connectionId, bucket, query, parsed]);

  const parentPrefix = (key: string): string => {
    const idx = key.lastIndexOf("/");
    if (idx === -1) return "";
    return key.slice(0, idx + 1);
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Sparkles className="h-4 w-4 text-primary" />
            Smart bucket search
          </DialogTitle>
        </DialogHeader>

        {indexStale && (
          <p className="text-xs text-amber-600 dark:text-amber-500">
            Index may be outdated after recent changes. Rebuild from the Index bucket dialog.
          </p>
        )}

        <div className="flex gap-2">
          <div className="relative min-w-0 flex-1">
            <Search className="pointer-events-none absolute left-2.5 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
            <Input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") void runSearch();
              }}
              placeholder="e.g. pdf files larger than 10mb last 30 days"
              className="pl-8"
              autoFocus
            />
          </div>
          <Button onClick={() => void runSearch()} disabled={loading || !query.trim()}>
            {loading ? <Loader2 className="h-4 w-4 animate-spin" /> : "Search"}
          </Button>
        </div>

        {(parsing || parsed) && query.trim() && (
          <p className="text-xs text-muted-foreground">
            {parsing ? (
              "Interpreting…"
            ) : (
              <>
                <span className="font-medium text-foreground">Interpreted:</span>{" "}
                {parsed?.summary}
                {parsed?.confidence === "low" && " (simple text match)"}
              </>
            )}
          </p>
        )}

        <ScrollArea className="h-72 rounded-md border">
          {loading && (
            <div className="flex items-center gap-2 p-4 text-sm text-muted-foreground">
              <Loader2 className="h-4 w-4 animate-spin" />
              Searching…
            </div>
          )}
          {!loading && searched && results.length === 0 && (
            <p className="p-4 text-sm text-muted-foreground">No matches.</p>
          )}
          {!loading &&
            results.map((item) => (
              <button
                key={item.key}
                type="button"
                className="flex w-full items-start justify-between gap-3 border-b px-3 py-2 text-left text-sm hover:bg-muted/50 last:border-b-0"
                onClick={() => {
                  onNavigate(parentPrefix(item.key));
                  onOpenChange(false);
                }}
              >
                <span className="min-w-0 break-all font-mono text-xs">{item.key}</span>
                <span className="shrink-0 font-mono text-xs text-muted-foreground tabular-nums">
                  {formatBytes(item.size)}
                </span>
              </button>
            ))}
        </ScrollArea>

        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Close
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
