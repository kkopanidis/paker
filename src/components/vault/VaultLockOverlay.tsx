import { useState } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  formatIpcError,
  resetMasterKeyWithOsAuth,
  unlockVault,
} from "@/lib/tauri";
import type { VaultStatus } from "@/types/vault";

interface VaultLockOverlayProps {
  status: VaultStatus;
  onUnlocked: () => void;
}

export function VaultLockOverlay({ status, onUnlocked }: VaultLockOverlayProps) {
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [showReset, setShowReset] = useState(false);
  const [newPassword, setNewPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");

  const blocked = status.unlockBlockedSecs > 0;

  const handleUnlock = async () => {
    if (!password) return;
    setBusy(true);
    try {
      await unlockVault(password);
      setPassword("");
      onUnlocked();
    } catch (error) {
      toast.error(formatIpcError(error));
    } finally {
      setBusy(false);
    }
  };

  const handleOsReset = async () => {
    if (newPassword.length < 8) {
      toast.error("Master key must be at least 8 characters.");
      return;
    }
    if (newPassword !== confirmPassword) {
      toast.error("Master keys do not match.");
      return;
    }
    setBusy(true);
    try {
      await resetMasterKeyWithOsAuth(newPassword);
      setNewPassword("");
      setConfirmPassword("");
      setShowReset(false);
      onUnlocked();
      toast.success("Master key reset successfully.");
    } catch (error) {
      toast.error(formatIpcError(error));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="fixed inset-0 z-[100] flex items-center justify-center bg-background/95 backdrop-blur-sm">
      <div className="w-full max-w-sm space-y-6 rounded-lg border bg-card p-6 shadow-lg">
        <div className="space-y-1 text-center">
          <h2 className="text-lg font-semibold">Paker is locked</h2>
          <p className="text-sm text-muted-foreground">
            Enter your master key to access connection secrets.
          </p>
        </div>

        {!showReset ? (
          <div className="space-y-4">
            <div className="grid gap-2">
              <Label htmlFor="vault-unlock-password">Master key</Label>
              <Input
                id="vault-unlock-password"
                type="password"
                autoComplete="current-password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                disabled={busy || blocked}
                onKeyDown={(e) => {
                  if (e.key === "Enter") void handleUnlock();
                }}
              />
            </div>
            {blocked ? (
              <p className="text-sm text-destructive">
                Too many failed attempts. Try again in {status.unlockBlockedSecs} seconds.
              </p>
            ) : null}
            <Button
              className="w-full"
              onClick={() => void handleUnlock()}
              disabled={busy || blocked || !password}
            >
              Unlock
            </Button>
            {status.recoveryAvailable ? (
              <Button
                variant="link"
                className="w-full text-sm"
                onClick={() => setShowReset(true)}
                disabled={busy}
              >
                Forgot master key? Reset with system authentication
              </Button>
            ) : null}
          </div>
        ) : (
          <div className="space-y-4">
            <p className="text-sm text-muted-foreground">
              Authenticate with your device (Touch ID, Windows Hello, or password) to set a
              new master key.
            </p>
            <div className="grid gap-2">
              <Label htmlFor="vault-reset-password">New master key</Label>
              <Input
                id="vault-reset-password"
                type="password"
                autoComplete="new-password"
                value={newPassword}
                onChange={(e) => setNewPassword(e.target.value)}
                disabled={busy}
              />
            </div>
            <div className="grid gap-2">
              <Label htmlFor="vault-reset-confirm">Confirm new master key</Label>
              <Input
                id="vault-reset-confirm"
                type="password"
                autoComplete="new-password"
                value={confirmPassword}
                onChange={(e) => setConfirmPassword(e.target.value)}
                disabled={busy}
              />
            </div>
            <div className="flex gap-2">
              <Button variant="outline" className="flex-1" onClick={() => setShowReset(false)} disabled={busy}>
                Back
              </Button>
              <Button className="flex-1" onClick={() => void handleOsReset()} disabled={busy}>
                Reset
              </Button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
