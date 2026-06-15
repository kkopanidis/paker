import { useEffect, useState } from "react";
import { Loader2 } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { ScrollArea } from "@/components/ui/scroll-area";
import { assistantListProposals, formatIpcError } from "@/lib/tauri";
import type { ProposalEntry } from "@/types/assistant";

interface ProposalAuditDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  connectionId: string | null;
}

const STATUS_VARIANT: Record<string, "default" | "secondary" | "destructive" | "outline"> = {
  pending: "secondary",
  executed: "default",
  rejected: "destructive",
};

export function ProposalAuditDialog({
  open,
  onOpenChange,
  connectionId,
}: ProposalAuditDialogProps) {
  const [items, setItems] = useState<ProposalEntry[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;

    let cancelled = false;
    setLoading(true);
    setError(null);

    void assistantListProposals(connectionId ?? undefined)
      .then((rows) => {
        if (!cancelled) setItems(rows);
      })
      .catch((err) => {
        if (!cancelled) setError(formatIpcError(err));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [open, connectionId]);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>Action history</DialogTitle>
        </DialogHeader>

        {loading && (
          <div className="flex items-center gap-2 text-sm text-muted-foreground">
            <Loader2 className="h-4 w-4 animate-spin" />
            Loading proposals…
          </div>
        )}

        {error && (
          <p className="rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-sm text-destructive">
            {error}
          </p>
        )}

        {!loading && !error && items.length === 0 && (
          <p className="text-sm text-muted-foreground">No proposals recorded yet.</p>
        )}

        {!loading && items.length > 0 && (
          <ScrollArea className="h-72">
            <table className="w-full text-xs">
              <thead className="sticky top-0 bg-muted/80">
                <tr>
                  <th className="px-2 py-1 text-left">When</th>
                  <th className="px-2 py-1 text-left">Kind</th>
                  <th className="px-2 py-1 text-left">Bucket</th>
                  <th className="px-2 py-1 text-left">Status</th>
                </tr>
              </thead>
              <tbody>
                {items.map((item) => (
                  <tr key={item.id} className="border-t">
                    <td className="whitespace-nowrap px-2 py-1">{item.createdAt}</td>
                    <td className="px-2 py-1">{item.kind}</td>
                    <td className="max-w-[10rem] truncate px-2 py-1 font-mono">{item.bucket}</td>
                    <td className="px-2 py-1">
                      <Badge variant={STATUS_VARIANT[item.status] ?? "outline"}>{item.status}</Badge>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </ScrollArea>
        )}

        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Close
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
