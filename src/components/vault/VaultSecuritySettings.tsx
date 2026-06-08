import { useEffect, useState } from "react";
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
import {
  changeMasterKey,
  formatIpcError,
  getVaultStatus,
  lockVault,
  setVaultPreferences,
  setupVault,
} from "@/lib/tauri";
import type { VaultStatus } from "@/types/vault";

interface VaultSecuritySettingsProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onChanged: () => void;
}

export function VaultSecuritySettings({
  open,
  onOpenChange,
  onChanged,
}: VaultSecuritySettingsProps) {
  const [status, setStatus] = useState<VaultStatus | null>(null);
  const [autoLockMinutes, setAutoLockMinutes] = useState(15);
  const [lockOnBlur, setLockOnBlur] = useState(false);
  const [setupPassword, setSetupPassword] = useState("");
  const [setupConfirm, setSetupConfirm] = useState("");
  const [currentPassword, setCurrentPassword] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [newPasswordConfirm, setNewPasswordConfirm] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!open) return;
    void getVaultStatus()
      .then((next) => {
        setStatus(next);
        setAutoLockMinutes(next.autoLockMinutes);
        setLockOnBlur(next.lockOnBlur);
      })
      .catch(() => setStatus(null));
  }, [open]);

  const handleSavePreferences = async () => {
    if (!status?.enabled) return;
    setBusy(true);
    try {
      await setVaultPreferences({ autoLockMinutes, lockOnBlur });
      onChanged();
      toast.success("Security preferences saved.");
    } catch (error) {
      toast.error(formatIpcError(error));
    } finally {
      setBusy(false);
    }
  };

  const handleEnableVault = async () => {
    if (setupPassword.length < 8) {
      toast.error("Master key must be at least 8 characters.");
      return;
    }
    if (setupPassword !== setupConfirm) {
      toast.error("Master keys do not match.");
      return;
    }
    setBusy(true);
    try {
      await setupVault({
        masterPassword: setupPassword,
        autoLockMinutes,
        lockOnBlur,
      });
      setSetupPassword("");
      setSetupConfirm("");
      onChanged();
      toast.success("Vault enabled.");
    } catch (error) {
      toast.error(formatIpcError(error));
    } finally {
      setBusy(false);
    }
  };

  const handleChangePassword = async () => {
    if (newPassword.length < 8) {
      toast.error("Master key must be at least 8 characters.");
      return;
    }
    if (newPassword !== newPasswordConfirm) {
      toast.error("Master keys do not match.");
      return;
    }
    setBusy(true);
    try {
      await changeMasterKey({
        currentPassword,
        newPassword,
      });
      setCurrentPassword("");
      setNewPassword("");
      setNewPasswordConfirm("");
      toast.success("Master key changed.");
    } catch (error) {
      toast.error(formatIpcError(error));
    } finally {
      setBusy(false);
    }
  };

  const handleLockNow = async () => {
    setBusy(true);
    try {
      await lockVault();
      onChanged();
      onOpenChange(false);
    } catch (error) {
      toast.error(formatIpcError(error));
    } finally {
      setBusy(false);
    }
  };

  const enabled = status?.enabled === true;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md max-h-[85vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>Security</DialogTitle>
          <DialogDescription>
            Manage vault protection for stored connection secrets.
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-6 py-2">
          <div className="space-y-3">
            <Label htmlFor="auto-lock-minutes">Auto-lock after inactivity (minutes, 0 = off)</Label>
            <Input
              id="auto-lock-minutes"
              type="number"
              min={0}
              max={1440}
              value={autoLockMinutes}
              onChange={(e) => setAutoLockMinutes(Number(e.target.value) || 0)}
              disabled={busy}
            />
            <label className="flex items-center gap-2 text-sm">
              <input
                type="checkbox"
                checked={lockOnBlur}
                onChange={(e) => setLockOnBlur(e.target.checked)}
                disabled={busy}
              />
              Lock when window loses focus
            </label>
            {enabled ? (
              <Button variant="secondary" size="sm" onClick={() => void handleSavePreferences()} disabled={busy}>
                Save preferences
              </Button>
            ) : null}
          </div>

          {!enabled ? (
            <div className="space-y-3 border-t pt-4">
              <p className="text-sm font-medium">Enable vault</p>
              <div className="grid gap-2">
                <Label htmlFor="enable-vault-password">Master key</Label>
                <Input
                  id="enable-vault-password"
                  type="password"
                  autoComplete="new-password"
                  value={setupPassword}
                  onChange={(e) => setSetupPassword(e.target.value)}
                  disabled={busy}
                />
              </div>
              <div className="grid gap-2">
                <Label htmlFor="enable-vault-confirm">Confirm master key</Label>
                <Input
                  id="enable-vault-confirm"
                  type="password"
                  autoComplete="new-password"
                  value={setupConfirm}
                  onChange={(e) => setSetupConfirm(e.target.value)}
                  disabled={busy}
                />
              </div>
              <Button onClick={() => void handleEnableVault()} disabled={busy}>
                Enable vault
              </Button>
            </div>
          ) : (
            <div className="space-y-3 border-t pt-4">
              <p className="text-sm font-medium">Change master key</p>
              <div className="grid gap-2">
                <Label htmlFor="current-vault-password">Current master key</Label>
                <Input
                  id="current-vault-password"
                  type="password"
                  autoComplete="current-password"
                  value={currentPassword}
                  onChange={(e) => setCurrentPassword(e.target.value)}
                  disabled={busy}
                />
              </div>
              <div className="grid gap-2">
                <Label htmlFor="new-vault-password">New master key</Label>
                <Input
                  id="new-vault-password"
                  type="password"
                  autoComplete="new-password"
                  value={newPassword}
                  onChange={(e) => setNewPassword(e.target.value)}
                  disabled={busy}
                />
              </div>
              <div className="grid gap-2">
                <Label htmlFor="new-vault-confirm">Confirm new master key</Label>
                <Input
                  id="new-vault-confirm"
                  type="password"
                  autoComplete="new-password"
                  value={newPasswordConfirm}
                  onChange={(e) => setNewPasswordConfirm(e.target.value)}
                  disabled={busy}
                />
              </div>
              <Button variant="secondary" onClick={() => void handleChangePassword()} disabled={busy}>
                Change master key
              </Button>
              <Button variant="outline" onClick={() => void handleLockNow()} disabled={busy}>
                Lock now
              </Button>
            </div>
          )}
        </div>

        <DialogFooter>
          <Button variant="ghost" onClick={() => onOpenChange(false)} disabled={busy}>
            Close
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
