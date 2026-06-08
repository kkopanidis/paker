import { Shield } from "lucide-react";
import { useState } from "react";
import { VaultSecuritySettings } from "@/components/vault/VaultSecuritySettings";
import { Button } from "@/components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";

export function SecurityToggle() {
  const [open, setOpen] = useState(false);
  const [revision, setRevision] = useState(0);

  return (
    <>
      <Tooltip>
        <TooltipTrigger asChild>
          <Button
            variant="ghost"
            size="icon"
            className="h-8 w-8"
            onClick={() => setOpen(true)}
            aria-label="Security settings"
          >
            <Shield className="h-4 w-4" />
          </Button>
        </TooltipTrigger>
        <TooltipContent>Security</TooltipContent>
      </Tooltip>
      <VaultSecuritySettings
        key={revision}
        open={open}
        onOpenChange={setOpen}
        onChanged={() => setRevision((r) => r + 1)}
      />
    </>
  );
}
