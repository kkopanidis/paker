import { useState } from "react";
import { toast } from "sonner";
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
import { formatIpcError, setupVault } from "@/lib/tauri";

interface VaultSetupDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onComplete: () => void;
  onSkip: () => void;
}

export function VaultSetupDialog({
  open,
  onOpenChange,
  onComplete,
  onSkip,
}: VaultSetupDialogProps) {
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [busy, setBusy] = useState(false);

  const handleEnable = async () => {
    if (password.length < 8) {
      toast.error("Master key must be at least 8 characters.");
      return;
    }
    if (password !== confirm) {
      toast.error("Master keys do not match.");
      return;
    }
    setBusy(true);
    try {
      await setupVault({
        masterPassword: password,
        autoLockMinutes: 15,
        lockOnBlur: false,
      });
      setPassword("");
      setConfirm("");
      onComplete();
      onOpenChange(false);
      toast.success("Vault enabled. Your connection secrets are now protected.");
    } catch (error) {
      toast.error(formatIpcError(error));
    } finally {
      setBusy(false);
    }
  };

  const handleSkip = () => {
    onSkip();
    onOpenChange(false);
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Protect your connection secrets</DialogTitle>
          <DialogDescription>
            Set a master key to encrypt stored credentials. You can enable this later
            from security settings. Skipping keeps the current storage behavior.
          </DialogDescription>
        </DialogHeader>
        <div className="grid gap-4 py-2">
          <div className="grid gap-2">
            <Label htmlFor="vault-setup-password">Master key</Label>
            <Input
              id="vault-setup-password"
              type="password"
              autoComplete="new-password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              disabled={busy}
            />
          </div>
          <div className="grid gap-2">
            <Label htmlFor="vault-setup-confirm">Confirm master key</Label>
            <Input
              id="vault-setup-confirm"
              type="password"
              autoComplete="new-password"
              value={confirm}
              onChange={(e) => setConfirm(e.target.value)}
              disabled={busy}
              onKeyDown={(e) => {
                if (e.key === "Enter") void handleEnable();
              }}
            />
          </div>
        </div>
        <DialogFooter className="gap-2 sm:gap-0">
          <Button variant="ghost" onClick={handleSkip} disabled={busy}>
            Skip for now
          </Button>
          <Button onClick={() => void handleEnable()} disabled={busy}>
            Enable vault
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
