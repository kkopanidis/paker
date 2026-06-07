import { useEffect, useState } from "react";
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

interface BookmarkDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  defaultLabel: string;
  onConfirm: (label: string) => void;
}

export function BookmarkDialog({
  open,
  onOpenChange,
  defaultLabel,
  onConfirm,
}: BookmarkDialogProps) {
  const [label, setLabel] = useState(defaultLabel);

  useEffect(() => {
    if (open) {
      setLabel(defaultLabel);
    }
  }, [open, defaultLabel]);

  const handleConfirm = () => {
    const trimmed = label.trim();
    if (!trimmed) return;
    onConfirm(trimmed);
    onOpenChange(false);
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Bookmark location</DialogTitle>
        </DialogHeader>
        <div className="space-y-2 py-2">
          <Label htmlFor="bookmark-label">Name</Label>
          <Input
            id="bookmark-label"
            value={label}
            onChange={(event) => setLabel(event.target.value)}
            placeholder="Bookmark name"
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                event.preventDefault();
                handleConfirm();
              }
            }}
            autoFocus
          />
        </div>
        <DialogFooter>
          <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button type="button" disabled={!label.trim()} onClick={handleConfirm}>
            Save bookmark
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
