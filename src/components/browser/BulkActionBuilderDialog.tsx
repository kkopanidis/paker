import { useState } from "react";
import { Loader2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { assistantBuildProposal, assistantParseQuery, formatIpcError } from "@/lib/tauri";
import type { ActionProposal, BuildProposalInput } from "@/types/assistant";

interface BulkActionBuilderDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  connectionId: string | null;
  bucket: string | null;
  onProposalReady: (proposal: ActionProposal) => void;
}

type Tab = "delete" | "rename" | "sync";

export function BulkActionBuilderDialog({
  open,
  onOpenChange,
  connectionId,
  bucket,
  onProposalReady,
}: BulkActionBuilderDialogProps) {
  const [tab, setTab] = useState<Tab>("delete");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [deleteQueryText, setDeleteQueryText] = useState("");
  const [sourcePattern, setSourcePattern] = useState("");
  const [destTemplate, setDestTemplate] = useState("");
  const [copyOnly, setCopyOnly] = useState(false);
  const [sourcePrefix, setSourcePrefix] = useState("");
  const [destPrefix, setDestPrefix] = useState("");
  const [syncMode, setSyncMode] = useState<"addOnly" | "mirror">("addOnly");

  const reset = () => {
    setError(null);
    setDeleteQueryText("");
    setSourcePattern("");
    setDestTemplate("");
    setCopyOnly(false);
    setSourcePrefix("");
    setDestPrefix("");
    setSyncMode("addOnly");
    setTab("delete");
  };

  const handleClose = (next: boolean) => {
    if (!next) reset();
    onOpenChange(next);
  };

  const buildDeleteInput = async (): Promise<BuildProposalInput> => {
    if (!connectionId || !bucket) throw new Error("No bucket selected");
    const parsed = await assistantParseQuery(deleteQueryText.trim());
    return {
      kind: "deleteByQuery",
      connectionId,
      bucket,
      query: parsed.query,
      dryRun: false,
    };
  };

  const buildRenameInput = (): BuildProposalInput => {
    if (!connectionId || !bucket) throw new Error("No bucket selected");
    return {
      kind: "renamePattern",
      connectionId,
      bucket,
      sourcePattern: sourcePattern.trim(),
      destTemplate: destTemplate.trim(),
      copyOnly,
    };
  };

  const buildSyncInput = (): BuildProposalInput => {
    if (!connectionId || !bucket) throw new Error("No bucket selected");
    return {
      kind: "syncPlan",
      connectionId,
      bucket,
      sourcePrefix: sourcePrefix.trim(),
      destPrefix: destPrefix.trim(),
      mode: syncMode,
      generateCli: true,
    };
  };

  const handlePreview = async () => {
    if (!connectionId || !bucket) return;
    setBusy(true);
    setError(null);
    try {
      let input: BuildProposalInput;
      if (tab === "delete") {
        if (!deleteQueryText.trim()) {
          setError("Enter a search query describing objects to delete.");
          return;
        }
        input = await buildDeleteInput();
      } else if (tab === "rename") {
        if (!sourcePattern.trim() || !destTemplate.trim()) {
          setError("Source pattern and destination template are required.");
          return;
        }
        input = buildRenameInput();
      } else {
        if (!sourcePrefix.trim() || !destPrefix.trim()) {
          setError("Source and destination prefixes are required.");
          return;
        }
        input = buildSyncInput();
      }

      const proposal = await assistantBuildProposal(input);
      onProposalReady(proposal);
      handleClose(false);
    } catch (err) {
      setError(formatIpcError(err));
    } finally {
      setBusy(false);
    }
  };

  const tabButton = (id: Tab, label: string) => (
    <Button
      type="button"
      variant={tab === id ? "secondary" : "ghost"}
      size="sm"
      onClick={() => setTab(id)}
    >
      {label}
    </Button>
  );

  return (
    <Dialog open={open} onOpenChange={handleClose}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle>Bulk actions</DialogTitle>
        </DialogHeader>

        <div className="flex gap-1 border-b pb-2">
          {tabButton("delete", "Delete by query")}
          {tabButton("rename", "Rename pattern")}
          {tabButton("sync", "Sync plan")}
        </div>

        {tab === "delete" && (
          <div className="space-y-2">
            <Label htmlFor="delete-query">Natural language or filter query</Label>
            <Input
              id="delete-query"
              placeholder='e.g. "*.log" older than 90 days in logs/'
              value={deleteQueryText}
              onChange={(e) => setDeleteQueryText(e.target.value)}
            />
            <p className="text-xs text-muted-foreground">
              Parsed against the local bucket index. Review the proposal before anything is deleted.
            </p>
          </div>
        )}

        {tab === "rename" && (
          <div className="space-y-3">
            <div className="space-y-2">
              <Label htmlFor="source-pattern">Source glob pattern</Label>
              <Input
                id="source-pattern"
                placeholder="logs/**/*.log"
                value={sourcePattern}
                onChange={(e) => setSourcePattern(e.target.value)}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="dest-template">Destination template</Label>
              <Input
                id="dest-template"
                placeholder="archive/{1}.log"
                value={destTemplate}
                onChange={(e) => setDestTemplate(e.target.value)}
              />
            </div>
            <label className="flex items-center gap-2 text-sm">
              <input
                type="checkbox"
                checked={copyOnly}
                onChange={(e) => setCopyOnly(e.target.checked)}
              />
              Copy only (keep originals)
            </label>
          </div>
        )}

        {tab === "sync" && (
          <div className="space-y-3">
            <div className="space-y-2">
              <Label htmlFor="source-prefix">Source prefix</Label>
              <Input
                id="source-prefix"
                placeholder="staging/"
                value={sourcePrefix}
                onChange={(e) => setSourcePrefix(e.target.value)}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="dest-prefix">Destination prefix</Label>
              <Input
                id="dest-prefix"
                placeholder="production/"
                value={destPrefix}
                onChange={(e) => setDestPrefix(e.target.value)}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="sync-mode">Mode</Label>
              <select
                id="sync-mode"
                className="flex h-9 w-full rounded-md border bg-background px-3 text-sm"
                value={syncMode}
                onChange={(e) => setSyncMode(e.target.value as "addOnly" | "mirror")}
              >
                <option value="addOnly">Add only — copy missing keys</option>
                <option value="mirror">Mirror — copy and delete extras</option>
              </select>
            </div>
          </div>
        )}

        {error && (
          <p className="rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-sm text-destructive">
            {error}
          </p>
        )}

        <DialogFooter>
          <Button variant="outline" onClick={() => handleClose(false)} disabled={busy}>
            Cancel
          </Button>
          <Button onClick={() => void handlePreview()} disabled={busy || !connectionId || !bucket}>
            {busy ? <Loader2 className="h-4 w-4 animate-spin" /> : "Preview proposal"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
