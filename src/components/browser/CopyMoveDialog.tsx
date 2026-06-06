import { useState } from "react";
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
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { cn } from "@/lib/utils";
import type { BucketInfo } from "@/types/s3";

export type CopyMoveMode = "copy" | "move";

interface CopyMoveDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  buckets: BucketInfo[];
  currentBucket: string | null;
  itemCount: number;
  initialMode?: CopyMoveMode;
  busy?: boolean;
  onConfirm: (destBucket: string, destPrefix: string, mode: CopyMoveMode) => void;
}

export function CopyMoveDialog({
  open,
  onOpenChange,
  buckets,
  currentBucket,
  itemCount,
  initialMode = "copy",
  busy,
  onConfirm,
}: CopyMoveDialogProps) {
  const [mode, setMode] = useState<CopyMoveMode>(initialMode);
  const [destBucket, setDestBucket] = useState<string>("");
  const [destPrefix, setDestPrefix] = useState("");

  const effectiveBucket = destBucket || currentBucket || "";

  const handleOpenChange = (nextOpen: boolean) => {
    if (!nextOpen) {
      setDestBucket("");
      setDestPrefix("");
      setMode(initialMode);
    }
    onOpenChange(nextOpen);
  };

  const handleConfirm = () => {
    if (!effectiveBucket) return;
    onConfirm(effectiveBucket, destPrefix.trim(), mode);
  };

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent className="max-w-sm">
        <DialogHeader>
          <DialogTitle>
            {mode === "copy" ? "Copy" : "Move"} {itemCount}{" "}
            {itemCount === 1 ? "item" : "items"}
          </DialogTitle>
        </DialogHeader>

        <div className="space-y-4">
          <div className="flex gap-2">
            <Button
              type="button"
              variant="outline"
              size="sm"
              className={cn(mode === "copy" && "border-primary bg-primary/10 text-primary")}
              onClick={() => setMode("copy")}
            >
              Copy
            </Button>
            <Button
              type="button"
              variant="outline"
              size="sm"
              className={cn(mode === "move" && "border-primary bg-primary/10 text-primary")}
              onClick={() => setMode("move")}
            >
              Move
            </Button>
          </div>

          <div className="space-y-1.5">
            <Label htmlFor="dest-bucket">Destination bucket</Label>
            <Select value={effectiveBucket} onValueChange={(v) => setDestBucket(v)}>
              <SelectTrigger id="dest-bucket">
                <SelectValue placeholder="Select bucket…" />
              </SelectTrigger>
              <SelectContent>
                {buckets.map((b) => (
                  <SelectItem key={b.name} value={b.name}>
                    {b.name}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          <div className="space-y-1.5">
            <Label htmlFor="dest-prefix">Destination folder (optional)</Label>
            <Input
              id="dest-prefix"
              placeholder="e.g. backups/2024/"
              value={destPrefix}
              onChange={(e) => setDestPrefix(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && !busy && handleConfirm()}
            />
          </div>
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={() => handleOpenChange(false)} disabled={busy}>
            Cancel
          </Button>
          <Button onClick={handleConfirm} disabled={busy || !effectiveBucket}>
            {busy
              ? mode === "copy"
                ? "Copying…"
                : "Moving…"
              : mode === "copy"
                ? "Copy"
                : "Move"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
