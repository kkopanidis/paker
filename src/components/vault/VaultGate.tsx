import { useVault } from "@/hooks/useVault";
import { VaultLockOverlay } from "./VaultLockOverlay";
import { VaultSetupDialog } from "./VaultSetupDialog";

interface VaultGateProps {
  children: React.ReactNode;
}

export function VaultGate({ children }: VaultGateProps) {
  const vault = useVault();

  if (vault.loading) {
    return (
      <div className="flex h-screen items-center justify-center text-muted-foreground">
        Loading…
      </div>
    );
  }

  return (
    <>
      {!vault.showLockOverlay ? children : null}

      {vault.showSetupPrompt ? (
        <VaultSetupDialog
          open
          onOpenChange={() => {}}
          onComplete={() => void vault.refresh()}
          onSkip={() => void vault.skipSetupPrompt()}
        />
      ) : null}

      {vault.showLockOverlay && vault.status ? (
        <VaultLockOverlay status={vault.status} onUnlocked={() => void vault.refresh()} />
      ) : null}
    </>
  );
}
