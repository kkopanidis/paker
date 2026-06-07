import { useEffect, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { X } from "lucide-react";
import { checkForUpdate } from "@/lib/tauri";
import type { UpdateInfo } from "@/types/ui";
import { Button } from "@/components/ui/button";

interface UpdateBannerProps {
  enabled: boolean;
}

export function UpdateBanner({ enabled }: UpdateBannerProps) {
  const [update, setUpdate] = useState<UpdateInfo | null>(null);
  const [dismissed, setDismissed] = useState(false);

  useEffect(() => {
    if (!enabled) return;

    let cancelled = false;
    void checkForUpdate()
      .then((info) => {
        if (!cancelled && info.updateAvailable) {
          setUpdate(info);
        }
      })
      .catch(() => {
        // Silent fail — backend should not throw for network errors.
      });

    return () => {
      cancelled = true;
    };
  }, [enabled]);

  if (!update || dismissed) {
    return null;
  }

  const label = update.releaseName || `v${update.latestVersion}`;

  return (
    <div
      role="status"
      className="flex items-center justify-between gap-3 border-b bg-primary/10 px-4 py-2 text-sm"
    >
      <p className="min-w-0 text-foreground">
        <span className="font-medium">Update available:</span>{" "}
        <span className="text-muted-foreground">
          {label} (you have v{update.currentVersion})
        </span>
      </p>
      <div className="flex shrink-0 items-center gap-2">
        <Button
          size="sm"
          variant="default"
          onClick={() => void openUrl(update.releaseUrl)}
        >
          Download
        </Button>
        <Button
          size="sm"
          variant="ghost"
          className="h-8 w-8 p-0"
          aria-label="Dismiss update notification"
          onClick={() => setDismissed(true)}
        >
          <X className="h-4 w-4" />
        </Button>
      </div>
    </div>
  );
}
