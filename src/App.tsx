import { Toaster } from "sonner";
import { AppShell } from "@/components/layout/AppShell";
import { VaultGate } from "@/components/vault/VaultGate";
import { TooltipProvider } from "@/components/ui/tooltip";

function App() {
  return (
    <TooltipProvider>
      <VaultGate>
        <AppShell />
      </VaultGate>
      <Toaster richColors closeButton position="bottom-right" />
    </TooltipProvider>
  );
}

export default App;
