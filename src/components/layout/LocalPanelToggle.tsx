import { HardDrive } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";

interface LocalPanelToggleProps {
  open: boolean;
  onToggle: () => void;
}

export function LocalPanelToggle({ open, onToggle }: LocalPanelToggleProps) {
  return (
    <TooltipProvider>
      <Tooltip>
        <TooltipTrigger asChild>
          <Button
            variant={open ? "secondary" : "ghost"}
            size="icon"
            onClick={onToggle}
            aria-pressed={open}
            aria-label={open ? "Hide local files" : "Show local files"}
          >
            <HardDrive className="h-4 w-4" />
          </Button>
        </TooltipTrigger>
        <TooltipContent>{open ? "Hide local files" : "Show local files"}</TooltipContent>
      </Tooltip>
    </TooltipProvider>
  );
}
