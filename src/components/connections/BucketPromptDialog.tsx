import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

interface BucketPromptDialogProps {
  open: boolean;
  connectionName?: string;
  busy?: boolean;
  onOpenChange: (open: boolean) => void;
  onConnect: (bucket: string) => Promise<void>;
  onListAll: () => Promise<void>;
}

export function BucketPromptDialog({
  open,
  connectionName,
  busy,
  onOpenChange,
  onConnect,
  onListAll,
}: BucketPromptDialogProps) {
  const [bucket, setBucket] = useState("");

  useEffect(() => {
    if (open) setBucket("");
  }, [open]);

  const submitConnect = async () => {
    const name = bucket.trim();
    if (!name) return;
    await onConnect(name);
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle>Specify a bucket</DialogTitle>
          <DialogDescription>
            {connectionName ? (
              <>
                <span className="font-medium text-foreground">{connectionName}</span> has no
                bucket configured. Enter a bucket name to verify your credentials and connect,
                or list all buckets if your key has permission.
              </>
            ) : (
              <>
                Enter a bucket name to verify your credentials and connect, or list all buckets
                if your key has permission.
              </>
            )}
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-2">
          <Label htmlFor="bucket-name">Bucket name</Label>
          <Input
            id="bucket-name"
            value={bucket}
            onChange={(e) => setBucket(e.target.value)}
            placeholder="my-bucket"
            autoFocus
            onKeyDown={(e) => e.key === "Enter" && void submitConnect()}
          />
        </div>

        <DialogFooter className="flex-col gap-2 sm:flex-col sm:space-x-0">
          <Button
            className="w-full"
            disabled={busy || !bucket.trim()}
            onClick={() => void submitConnect()}
          >
            {busy ? "Verifying…" : "Connect to bucket"}
          </Button>
          <Button
            type="button"
            variant="outline"
            className="w-full"
            disabled={busy}
            onClick={() => void onListAll()}
          >
            List all buckets
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
