import { Toaster } from "sonner";
import { AppShell } from "@/components/layout/AppShell";
import { TooltipProvider } from "@/components/ui/tooltip";

function App() {
  return (
    <TooltipProvider>
      <AppShell />
      <Toaster richColors closeButton position="bottom-right" />
    </TooltipProvider>
  );
}

export default App;
