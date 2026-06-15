import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
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
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  assistantExecuteProposal,
  assistantRejectProposal,
  formatIpcError,
} from "@/lib/tauri";
import { formatBytes } from "@/lib/utils";
import type { ActionProposal, ExecutionResult } from "@/types/assistant";

interface ProposalReviewDialogProps {
  proposal: ActionProposal | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onComplete: (result: ExecutionResult | null) => void;
}

interface ProgressEvent {
  proposalId: string;
  done: number;
  total: number;
  phase?: string;
}

const KIND_LABEL: Record<string, string> = {
  deleteByQuery: "Delete by query",
  renamePattern: "Rename pattern",
  syncPlan: "Sync plan",
};

export function ProposalReviewDialog({
  proposal,
  open,
  onOpenChange,
  onComplete,
}: ProposalReviewDialogProps) {
  const [confirmText, setConfirmText] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [progress, setProgress] = useState<ProgressEvent | null>(null);

  const bucket = proposal?.bucket ?? "";
  const confirmed = confirmText === bucket;

  useEffect(() => {
    if (!open) {
      setConfirmText("");
      setError(null);
      setProgress(null);
      setBusy(false);
    }
  }, [open, proposal?.id]);

  useEffect(() => {
    if (!open || !proposal) return;

    let unlisten: (() => void) | undefined;
    void listen<ProgressEvent>("proposal://progress", (event) => {
      if (event.payload.proposalId === proposal.id) {
        setProgress(event.payload);
      }
    }).then((fn) => {
      unlisten = fn;
    });

    return () => {
      unlisten?.();
    };
  }, [open, proposal]);

  const handleReject = async () => {
    if (!proposal) return;
    setBusy(true);
    setError(null);
    try {
      await assistantRejectProposal(proposal.id, proposal.token);
      onComplete(null);
      onOpenChange(false);
    } catch (err) {
      setError(formatIpcError(err));
    } finally {
      setBusy(false);
    }
  };

  const handleApprove = async () => {
    if (!proposal || !confirmed) return;
    setBusy(true);
    setError(null);
    try {
      const result = await assistantExecuteProposal(proposal.id, proposal.token);
      onComplete(result);
      onOpenChange(false);
    } catch (err) {
      setError(formatIpcError(err));
    } finally {
      setBusy(false);
    }
  };

  if (!proposal) return null;

  const progressLabel =
    progress && progress.total > 0
      ? `${progress.done} / ${progress.total}`
      : progress?.phase === "complete"
        ? "Complete"
        : busy
          ? "Running…"
          : null;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>Review proposal</DialogTitle>
        </DialogHeader>

        <div className="space-y-3 text-sm">
          <div className="grid grid-cols-[8rem_1fr] gap-2">
            <span className="text-muted-foreground">Action</span>
            <span>{KIND_LABEL[proposal.kind] ?? proposal.kind}</span>
            <span className="text-muted-foreground">Bucket</span>
            <span className="font-mono">{proposal.bucket}</span>
            <span className="text-muted-foreground">Affected</span>
            <span>
              {proposal.totalAffected.toLocaleString()} objects · {formatBytes(proposal.totalBytes)}
            </span>
          </div>

          {proposal.warnings.length > 0 && (
            <div className="rounded-md border border-amber-500/40 bg-amber-500/10 px-3 py-2">
              <p className="mb-1 font-medium text-amber-800 dark:text-amber-200">Warnings</p>
              <ul className="list-inside list-disc space-y-0.5 text-amber-900 dark:text-amber-100">
                {proposal.warnings.map((w) => (
                  <li key={w}>{w}</li>
                ))}
              </ul>
            </div>
          )}

          <div>
            <p className="mb-1 text-muted-foreground">Preview (first {proposal.previewItems.length})</p>
            <ScrollArea className="h-40 rounded-md border">
              <table className="w-full text-xs">
                <thead className="sticky top-0 bg-muted/80">
                  <tr>
                    <th className="px-2 py-1 text-left">Key</th>
                    <th className="px-2 py-1 text-right">Size</th>
                    <th className="px-2 py-1 text-left">Action</th>
                  </tr>
                </thead>
                <tbody>
                  {proposal.previewItems.map((item) => (
                    <tr key={item.key} className="border-t">
                      <td className="max-w-[14rem] truncate px-2 py-1 font-mono">{item.key}</td>
                      <td className="px-2 py-1 text-right">{formatBytes(item.sizeBytes)}</td>
                      <td className="px-2 py-1">{item.actionDescription}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </ScrollArea>
          </div>

          <div className="space-y-2">
            <p className="text-muted-foreground">
              Type <span className="font-mono font-medium text-foreground">{bucket}</span> to confirm.
            </p>
            <Input
              value={confirmText}
              onChange={(e) => setConfirmText(e.target.value)}
              placeholder={bucket}
              disabled={busy}
              autoComplete="off"
            />
          </div>

          {progressLabel && (
            <p className="flex items-center gap-2 text-muted-foreground">
              <Loader2 className="h-4 w-4 animate-spin" />
              {progressLabel}
            </p>
          )}

          {error && (
            <p className="rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-destructive">
              {error}
            </p>
          )}
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={() => void handleReject()} disabled={busy}>
            Reject
          </Button>
          <Button
            variant="destructive"
            onClick={() => void handleApprove()}
            disabled={busy || !confirmed}
          >
            {busy ? "Executing…" : "Approve & execute"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
