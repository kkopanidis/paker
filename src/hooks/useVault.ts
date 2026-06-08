import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useCallback, useEffect, useRef, useState } from "react";
import {
  dismissVaultPrompt,
  formatIpcError,
  getVaultPromptDismissed,
  getVaultStatus,
  lockVault,
  recordVaultActivity,
} from "@/lib/tauri";
import type { VaultStatus } from "@/types/vault";

const ACTIVITY_THROTTLE_MS = 30_000;
const IDLE_CHECK_MS = 15_000;

export function useVault() {
  const [status, setStatus] = useState<VaultStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [promptDismissed, setPromptDismissed] = useState(true);
  const lastActivitySent = useRef(0);

  const refresh = useCallback(async () => {
    try {
      const [next, dismissed] = await Promise.all([
        getVaultStatus(),
        getVaultPromptDismissed(),
      ]);
      setStatus(next);
      setPromptDismissed(dismissed);
    } catch {
      setStatus(null);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    const unsubs: Array<() => void> = [];
    void listen("vault-locked", () => {
      void refresh();
    }).then((fn) => unsubs.push(fn));
    void listen("vault-unlocked", () => {
      void refresh();
    }).then((fn) => unsubs.push(fn));
    return () => {
      for (const fn of unsubs) fn();
    };
  }, [refresh]);

  const sendActivity = useCallback(() => {
    const now = Date.now();
    if (now - lastActivitySent.current < ACTIVITY_THROTTLE_MS) return;
    lastActivitySent.current = now;
    void recordVaultActivity().then(() => refresh()).catch(() => {});
  }, [refresh]);

  useEffect(() => {
    if (!status?.enabled || status.locked) return;

    const events = ["pointerdown", "keydown", "wheel", "scroll"] as const;
    const onActivity = () => sendActivity();
    for (const event of events) {
      window.addEventListener(event, onActivity, { capture: true, passive: true });
    }
    sendActivity();

    return () => {
      for (const event of events) {
        window.removeEventListener(event, onActivity, { capture: true });
      }
    };
  }, [status?.enabled, status?.locked, sendActivity]);

  useEffect(() => {
    if (!status?.enabled || status.locked || status.autoLockMinutes === 0) return;

    const id = window.setInterval(() => {
      void recordVaultActivity()
        .then(() => refresh())
        .catch(() => {});
    }, IDLE_CHECK_MS);

    return () => window.clearInterval(id);
  }, [status?.enabled, status?.locked, status?.autoLockMinutes, refresh]);

  useEffect(() => {
    if (!status?.enabled || status.locked || !status.lockOnBlur) return;

    let unlisten: (() => void) | undefined;
    void getCurrentWindow()
      .onFocusChanged(({ payload: focused }) => {
        if (!focused) {
          void lockVault()
            .then(() => refresh())
            .catch((error) => {
              console.error(formatIpcError(error));
            });
        }
      })
      .then((fn) => {
        unlisten = fn;
      });

    return () => {
      unlisten?.();
    };
  }, [status?.enabled, status?.locked, status?.lockOnBlur, refresh]);

  const skipSetupPrompt = useCallback(async () => {
    await dismissVaultPrompt();
    setPromptDismissed(true);
  }, []);

  const showSetupPrompt =
    !loading &&
    status?.setupRequired === true &&
    !status.enabled &&
    !promptDismissed;

  const showLockOverlay = !loading && status?.enabled === true && status.locked;

  return {
    status,
    loading,
    showSetupPrompt,
    showLockOverlay,
    refresh,
    skipSetupPrompt,
  };
}
