import { afterEach, describe, expect, it, vi } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { VaultStatus } from "@/types/vault";
import { useVault } from "./useVault";

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    onFocusChanged: vi.fn().mockResolvedValue(() => {}),
  }),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

afterEach(() => {
  clearMocks();
});

const unlockedStatus: VaultStatus = {
  enabled: true,
  locked: false,
  setupRequired: false,
  autoLockMinutes: 15,
  lockOnBlur: false,
  recoveryAvailable: true,
  unlockBlockedSecs: 0,
};

const lockedStatus: VaultStatus = {
  ...unlockedStatus,
  locked: true,
};

describe("useVault", () => {
  it("reports lock overlay when vault is enabled and locked", async () => {
    mockIPC((cmd) => {
      if (cmd === "get_vault_status") return lockedStatus;
      if (cmd === "get_vault_prompt_dismissed") return true;
    });

    const { result } = renderHook(() => useVault());

    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.showLockOverlay).toBe(true);
    expect(result.current.showSetupPrompt).toBe(false);
  });

  it("shows setup prompt when vault is not configured and prompt not dismissed", async () => {
    mockIPC((cmd) => {
      if (cmd === "get_vault_status") {
        return {
          enabled: false,
          locked: false,
          setupRequired: true,
          autoLockMinutes: 15,
          lockOnBlur: false,
          recoveryAvailable: true,
          unlockBlockedSecs: 0,
        } satisfies VaultStatus;
      }
      if (cmd === "get_vault_prompt_dismissed") return false;
    });

    const { result } = renderHook(() => useVault());

    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.showSetupPrompt).toBe(true);
    expect(result.current.showLockOverlay).toBe(false);
  });

  it("refreshes status after skip setup prompt", async () => {
    mockIPC((cmd, args) => {
      if (cmd === "get_vault_status") {
        return {
          enabled: false,
          locked: false,
          setupRequired: true,
          autoLockMinutes: 15,
          lockOnBlur: false,
          recoveryAvailable: true,
          unlockBlockedSecs: 0,
        } satisfies VaultStatus;
      }
      if (cmd === "get_vault_prompt_dismissed") return false;
      if (cmd === "dismiss_vault_prompt") {
        expect(args).toEqual({});
        return null;
      }
    });

    const { result } = renderHook(() => useVault());

    await waitFor(() => expect(result.current.showSetupPrompt).toBe(true));

    await act(async () => {
      await result.current.skipSetupPrompt();
    });

    expect(result.current.showSetupPrompt).toBe(false);
  });

  it("does not show lock overlay when vault is unlocked", async () => {
    mockIPC((cmd) => {
      if (cmd === "get_vault_status") return unlockedStatus;
      if (cmd === "get_vault_prompt_dismissed") return true;
      if (cmd === "record_vault_activity") return null;
    });

    const { result } = renderHook(() => useVault());

    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.showLockOverlay).toBe(false);
  });
});
