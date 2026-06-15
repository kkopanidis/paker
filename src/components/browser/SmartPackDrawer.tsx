import { useCallback, useEffect, useState } from "react";
import {
  BarChart3,
  Check,
  ChevronDown,
  ChevronUp,
  ClipboardCopy,
  Download,
  Loader2,
  Search,
  Sparkles,
  Terminal,
  Trash2,
  X,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  assistantGenerateCli,
  assistantGetBucketReport,
  assistantGetModelStatus,
  assistantOpenModelsFolder,
  assistantPackExport,
  assistantParseQuery,
  assistantParseQueryLlm,
  assistantQueryHistoryClear,
  assistantQueryHistoryInsert,
  assistantQueryHistoryList,
  assistantRunIndexQuery,
  formatIpcError,
} from "@/lib/tauri";
import { formatBytes } from "@/lib/utils";
import type {
  AssistantModelStatus,
  BucketReport,
  ParsedAssistantQuery,
  QueryHistoryItem,
} from "@/types/assistant";
import type { IndexedObject } from "@/types/s3";

interface SmartPackDrawerProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  connectionId: string | null;
  bucket: string | null;
  indexStale?: boolean;
  onNavigate: (prefix: string) => void;
}

const CONFIDENCE_COLOR: Record<string, string> = {
  high: "bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-200",
  medium: "bg-yellow-100 text-yellow-800 dark:bg-yellow-900 dark:text-yellow-200",
  low: "bg-slate-100 text-slate-700 dark:bg-slate-800 dark:text-slate-300",
};

function parentPrefix(key: string): string {
  const idx = key.lastIndexOf("/");
  if (idx === -1) return "";
  return key.slice(0, idx + 1);
}

export function SmartPackDrawer({
  open,
  onOpenChange,
  connectionId,
  bucket,
  indexStale,
  onNavigate,
}: SmartPackDrawerProps) {
  const [query, setQuery] = useState("");
  const [parsed, setParsed] = useState<ParsedAssistantQuery | null>(null);
  const [parsing, setParsing] = useState(false);
  const [results, setResults] = useState<IndexedObject[]>([]);
  const [loading, setLoading] = useState(false);
  const [searched, setSearched] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);

  const [selectedKeys, setSelectedKeys] = useState<Set<string>>(new Set());

  const [history, setHistory] = useState<QueryHistoryItem[]>([]);
  const [historyLoading, setHistoryLoading] = useState(false);

  const [report, setReport] = useState<BucketReport | null>(null);
  const [reportLoading, setReportLoading] = useState(false);
  const [reportOpen, setReportOpen] = useState(false);

  const [exporting, setExporting] = useState(false);
  const [copySuccess, setCopySuccess] = useState(false);
  const [cliCopied, setCliCopied] = useState(false);
  const [modelStatus, setModelStatus] = useState<AssistantModelStatus | null>(null);

  const refreshModelStatus = useCallback(async () => {
    try {
      setModelStatus(await assistantGetModelStatus());
    } catch {
      setModelStatus(null);
    }
  }, []);

  const refreshHistory = useCallback(async () => {
    if (!connectionId || !bucket) return;
    setHistoryLoading(true);
    try {
      const items = await assistantQueryHistoryList(connectionId, bucket, 10);
      setHistory(items);
    } catch {
      // non-fatal
    } finally {
      setHistoryLoading(false);
    }
  }, [connectionId, bucket]);

  const loadReport = useCallback(async () => {
    if (!connectionId || !bucket) return;
    setReportLoading(true);
    try {
      const data = await assistantGetBucketReport(connectionId, bucket, 8);
      setReport(data);
    } catch {
      setReport(null);
    } finally {
      setReportLoading(false);
    }
  }, [connectionId, bucket]);

  useEffect(() => {
    if (open) {
      void refreshHistory();
      void loadReport();
      void refreshModelStatus();
    } else {
      setQuery("");
      setParsed(null);
      setResults([]);
      setSearched(false);
      setSearchError(null);
      setSelectedKeys(new Set());
      setReport(null);
      setReportOpen(false);
    }
  }, [open, refreshHistory, loadReport, refreshModelStatus]);

  // Debounced regex-only parse as you type
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

  const resolveInterpretation = useCallback(
    async (text: string, current: ParsedAssistantQuery | null): Promise<ParsedAssistantQuery> => {
      const regex = current ?? (await assistantParseQuery(text));
      if (regex.confidence === "high") {
        return regex;
      }
      return assistantParseQueryLlm(text);
    },
    []
  );

  const runSearch = useCallback(async () => {
    if (!connectionId || !bucket || !query.trim()) return;

    setLoading(true);
    setSearched(true);
    setSearchError(null);
    setSelectedKeys(new Set());
    try {
      const interpretation = await resolveInterpretation(query.trim(), parsed);
      setParsed(interpretation);
      const hits = await assistantRunIndexQuery(connectionId, bucket, interpretation.query);
      setResults(hits);

      // Record in history (fire-and-forget)
      void assistantQueryHistoryInsert(
        connectionId,
        bucket,
        query.trim(),
        interpretation.summary,
        interpretation.confidence,
        hits.length
      ).then(() => void refreshHistory());
    } catch (err) {
      setResults([]);
      setSearchError(formatIpcError(err));
    } finally {
      setLoading(false);
    }
  }, [connectionId, bucket, query, parsed, refreshHistory, resolveInterpretation]);

  const applyHistoryChip = (item: QueryHistoryItem) => {
    setQuery(item.rawText);
    window.setTimeout(() => {
      void resolveInterpretation(item.rawText, null).then((p) => {
        setParsed(p);
        if (!connectionId || !bucket) return;
        setLoading(true);
        setSearched(true);
        setSearchError(null);
        setSelectedKeys(new Set());
        void assistantRunIndexQuery(connectionId, bucket, p.query)
          .then((hits) => {
            setResults(hits);
          })
          .catch((err) => {
            setResults([]);
            setSearchError(formatIpcError(err));
          })
          .finally(() => setLoading(false));
      });
    }, 0);
  };

  const clearHistory = async () => {
    if (!connectionId || !bucket) return;
    try {
      await assistantQueryHistoryClear(connectionId, bucket);
      setHistory([]);
    } catch {
      // non-fatal
    }
  };

  const toggleKey = (key: string) => {
    setSelectedKeys((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  };

  const selectAll = () => setSelectedKeys(new Set(results.map((r) => r.key)));
  const deselectAll = () => setSelectedKeys(new Set());

  const selectedList = results.filter((r) => selectedKeys.has(r.key));

  const copyKeys = async () => {
    const text = selectedList.map((r) => r.key).join("\n");
    await navigator.clipboard.writeText(text);
    setCopySuccess(true);
    window.setTimeout(() => setCopySuccess(false), 1500);
  };

  const exportCsv = async () => {
    if (!connectionId || !bucket) return;
    setExporting(true);
    try {
      await assistantPackExport(connectionId, bucket, selectedList.map((r) => r.key), "csv");
    } catch (err) {
      // Show nothing — dialog-cancelled silently ignored
      const msg = formatIpcError(err);
      if (!msg.toLowerCase().includes("cancelled")) {
        console.error("Export error:", msg);
      }
    } finally {
      setExporting(false);
    }
  };

  const copyCli = async () => {
    if (!connectionId || !bucket) return;
    try {
      const suggestions = await assistantGenerateCli({
        connectionId,
        bucket,
        prefix: "",
        keys: selectedList.map((r) => r.key),
      });
      if (suggestions.length > 0) {
        await navigator.clipboard.writeText(suggestions[0].command);
        setCliCopied(true);
        window.setTimeout(() => setCliCopied(false), 1500);
      }
    } catch {
      // non-fatal
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="flex max-h-[90vh] w-full max-w-3xl flex-col overflow-hidden p-0">
        {/* ── Header ──────────────────────────────────────────────── */}
        <DialogHeader className="flex-shrink-0 border-b px-5 py-4">
          <div className="flex items-center justify-between">
            <DialogTitle className="flex items-center gap-2 text-base font-semibold">
              <Sparkles className="h-4 w-4 text-primary" />
              Smart Pack
              {bucket && (
                <span className="ml-1 rounded-md bg-muted px-2 py-0.5 font-mono text-xs font-normal text-muted-foreground">
                  {bucket}
                </span>
              )}
            </DialogTitle>
          </div>
        </DialogHeader>

        <div className="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto px-5 py-4">
          {indexStale && (
            <p className="text-xs text-amber-600 dark:text-amber-500">
              Index may be outdated — rebuild from the Index bucket dialog.
            </p>
          )}

          {modelStatus && (
            <div className="flex flex-wrap items-center justify-between gap-2 rounded-md border bg-muted/30 px-3 py-2 text-xs">
              <p className="text-muted-foreground">{modelStatus.hint}</p>
              <Button
                type="button"
                variant="outline"
                size="sm"
                className="h-7 shrink-0"
                onClick={() => void assistantOpenModelsFolder()}
              >
                Open models folder
              </Button>
            </div>
          )}

          {/* ── Query bar ─────────────────────────────────────────── */}
          <div className="flex gap-2">
            <div className="relative min-w-0 flex-1">
              <Search className="pointer-events-none absolute left-2.5 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
              <Input
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") void runSearch();
                }}
                placeholder="e.g. pdf files larger than 10 MB from the last 30 days"
                className="pl-8"
                autoFocus
              />
            </div>
            <Button onClick={() => void runSearch()} disabled={loading || !query.trim()}>
              {loading ? <Loader2 className="h-4 w-4 animate-spin" /> : "Search"}
            </Button>
          </div>

          {/* Parse summary */}
          {(parsing || parsed) && query.trim() && (
            <p className="text-xs text-muted-foreground">
              {parsing ? (
                "Interpreting…"
              ) : (
                <>
                  <span className="font-medium text-foreground">Interpreted:</span>{" "}
                  {parsed?.summary}
                  {parsed && (
                    <span
                      className={`ml-1.5 inline-flex items-center rounded px-1.5 py-0.5 text-[10px] font-medium ${CONFIDENCE_COLOR[parsed.confidence] ?? ""}`}
                    >
                      {parsed.confidence}
                    </span>
                  )}
                </>
              )}
            </p>
          )}

          {/* ── History strip ─────────────────────────────────────── */}
          {(historyLoading || history.length > 0) && (
            <div className="flex items-center gap-2">
              <span className="shrink-0 text-xs text-muted-foreground">Recent:</span>
              <div className="flex min-w-0 flex-1 gap-1.5 overflow-x-auto pb-0.5">
                {historyLoading ? (
                  <Loader2 className="h-3 w-3 animate-spin text-muted-foreground" />
                ) : (
                  history.map((item) => (
                    <button
                      key={item.id}
                      type="button"
                      onClick={() => applyHistoryChip(item)}
                      className="inline-flex shrink-0 items-center gap-1 rounded-full border bg-muted/40 px-2 py-0.5 text-xs text-foreground hover:bg-muted"
                    >
                      {item.rawText.length > 32
                        ? `${item.rawText.slice(0, 32)}…`
                        : item.rawText}
                      <span className="text-[10px] text-muted-foreground">
                        ({item.resultCount})
                      </span>
                    </button>
                  ))
                )}
              </div>
              {history.length > 0 && (
                <button
                  type="button"
                  onClick={() => void clearHistory()}
                  className="shrink-0 text-muted-foreground hover:text-destructive"
                  aria-label="Clear history"
                >
                  <X className="h-3.5 w-3.5" />
                </button>
              )}
            </div>
          )}

          {/* ── Result list ───────────────────────────────────────── */}
          {(searched || loading) && (
            <div className="flex flex-col gap-1.5">
              {results.length > 0 && (
                <div className="flex items-center gap-2">
                  <span className="text-xs text-muted-foreground">
                    {results.length.toLocaleString()} result{results.length !== 1 ? "s" : ""}
                  </span>
                  <div className="flex gap-1 ml-auto">
                    <Button variant="outline" size="sm" className="h-6 text-xs px-2" onClick={selectAll}>
                      Select all
                    </Button>
                    {selectedKeys.size > 0 && (
                      <Button variant="outline" size="sm" className="h-6 text-xs px-2" onClick={deselectAll}>
                        Deselect
                      </Button>
                    )}
                  </div>
                </div>
              )}

              <ScrollArea className="h-56 rounded-md border">
                {loading && (
                  <div className="flex items-center gap-2 p-4 text-sm text-muted-foreground">
                    <Loader2 className="h-4 w-4 animate-spin" />
                    Searching…
                  </div>
                )}
                {!loading && searched && results.length === 0 && !searchError && (
                  <p className="p-4 text-sm text-muted-foreground">No matches.</p>
                )}
                {!loading && searchError && (
                  <p className="p-4 text-sm text-destructive">{searchError}</p>
                )}
                {!loading &&
                  results.map((item) => (
                    <div
                      key={item.key}
                      className="flex w-full items-center gap-2 border-b px-3 py-2 text-sm last:border-b-0 hover:bg-muted/40"
                    >
                      <Checkbox
                        checked={selectedKeys.has(item.key)}
                        onCheckedChange={() => toggleKey(item.key)}
                        aria-label={`Select ${item.key}`}
                      />
                      <button
                        type="button"
                        className="flex min-w-0 flex-1 items-start justify-between gap-3 text-left"
                        onClick={() => {
                          onNavigate(parentPrefix(item.key));
                          onOpenChange(false);
                        }}
                      >
                        <span className="min-w-0 break-all font-mono text-xs">{item.key}</span>
                        <span className="flex shrink-0 items-center gap-1.5 font-mono text-xs text-muted-foreground tabular-nums">
                          <span>{formatBytes(item.size)}</span>
                          {item.storageClass && item.storageClass !== "STANDARD" && (
                            <Badge variant="outline" className="py-0 text-[9px]">
                              {item.storageClass}
                            </Badge>
                          )}
                        </span>
                      </button>
                    </div>
                  ))}
              </ScrollArea>
            </div>
          )}

          {/* ── Pack actions bar ──────────────────────────────────── */}
          {selectedKeys.size > 0 && (
            <div className="flex flex-wrap items-center gap-2 rounded-md border bg-muted/30 px-3 py-2">
              <span className="text-xs font-medium text-foreground">
                {selectedKeys.size} selected
              </span>
              <div className="flex gap-1.5 ml-auto flex-wrap">
                <Button
                  variant="outline"
                  size="sm"
                  className="h-7 gap-1.5 text-xs"
                  onClick={() => void copyKeys()}
                >
                  {copySuccess ? (
                    <Check className="h-3.5 w-3.5 text-green-600" />
                  ) : (
                    <ClipboardCopy className="h-3.5 w-3.5" />
                  )}
                  Copy keys
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  className="h-7 gap-1.5 text-xs"
                  onClick={() => void exportCsv()}
                  disabled={exporting}
                >
                  {exporting ? (
                    <Loader2 className="h-3.5 w-3.5 animate-spin" />
                  ) : (
                    <Download className="h-3.5 w-3.5" />
                  )}
                  Export CSV
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  className="h-7 gap-1.5 text-xs"
                  onClick={() => void copyCli()}
                >
                  {cliCopied ? (
                    <Check className="h-3.5 w-3.5 text-green-600" />
                  ) : (
                    <Terminal className="h-3.5 w-3.5" />
                  )}
                  Copy as CLI
                </Button>
              </div>
            </div>
          )}

          {/* ── Report summary strip ──────────────────────────────── */}
          <div className="rounded-md border">
            <button
              type="button"
              onClick={() => setReportOpen((o) => !o)}
              className="flex w-full items-center justify-between px-3 py-2 text-sm font-medium hover:bg-muted/40"
            >
              <span className="flex items-center gap-1.5">
                <BarChart3 className="h-3.5 w-3.5 text-muted-foreground" />
                Bucket analysis
              </span>
              {reportLoading ? (
                <Loader2 className="h-3.5 w-3.5 animate-spin text-muted-foreground" />
              ) : reportOpen ? (
                <ChevronUp className="h-3.5 w-3.5 text-muted-foreground" />
              ) : (
                <ChevronDown className="h-3.5 w-3.5 text-muted-foreground" />
              )}
            </button>
            {reportOpen && report && (
              <div className="border-t px-3 py-3">
                <dl className="grid grid-cols-2 gap-x-4 gap-y-1.5 text-xs">
                  <dt className="text-muted-foreground">Objects</dt>
                  <dd className="tabular-nums">{report.totalObjects.toLocaleString()}</dd>
                  <dt className="text-muted-foreground">Total size</dt>
                  <dd className="tabular-nums">{formatBytes(report.totalBytes)}</dd>
                  <dt className="text-muted-foreground">Glacier-class</dt>
                  <dd className="tabular-nums">
                    {report.glacierObjectCount.toLocaleString()} ({formatBytes(report.glacierBytes)})
                  </dd>
                  <dt className="text-muted-foreground">
                    Small files (&lt; {formatBytes(report.smallFileThresholdBytes)})
                  </dt>
                  <dd className="tabular-nums">{report.smallFileCount.toLocaleString()}</dd>
                </dl>
                {report.topPrefixesByBytes.length > 0 && (
                  <div className="mt-3">
                    <p className="mb-1.5 text-xs font-medium">Top prefixes by size</p>
                    <div className="space-y-0.5">
                      {report.topPrefixesByBytes.slice(0, 5).map((row) => (
                        <div
                          key={row.prefix}
                          className="flex items-center justify-between gap-2 text-xs"
                        >
                          <span className="min-w-0 truncate font-mono text-muted-foreground">
                            {row.prefix}
                          </span>
                          <span className="shrink-0 tabular-nums text-muted-foreground">
                            {formatBytes(row.totalBytes)}
                          </span>
                        </div>
                      ))}
                    </div>
                  </div>
                )}
              </div>
            )}
            {reportOpen && !report && !reportLoading && (
              <p className="border-t px-3 py-2 text-xs text-muted-foreground">
                No index data available.
              </p>
            )}
          </div>
        </div>

        {/* ── Footer close ──────────────────────────────────────────── */}
        <div className="flex flex-shrink-0 justify-end border-t px-5 py-3">
          <Button variant="outline" size="sm" onClick={() => onOpenChange(false)}>
            Close
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
