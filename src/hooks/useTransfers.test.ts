import { afterEach, describe, expect, it, vi } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { TransferProgress } from "@/types/s3";
import { useTransfers } from "./useTransfers";

type ProgressHandler = (event: { payload: TransferProgress }) => void;

const progressHandlers = new Set<ProgressHandler>();

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(
    (event: string, handler: ProgressHandler): Promise<() => void> => {
      if (event === "transfer-progress") {
        progressHandlers.add(handler);
        return Promise.resolve(() => progressHandlers.delete(handler));
      }
      return Promise.resolve(() => undefined);
    }
  ),
}));

function emitTransferProgress(payload: TransferProgress) {
  for (const handler of progressHandlers) {
    handler({ payload });
  }
}

afterEach(() => {
  clearMocks();
  progressHandlers.clear();
  vi.clearAllMocks();
});

describe("useTransfers", () => {
  it("tracks transfer-progress events and optimistic cancel", async () => {
    mockIPC((cmd) => {
      if (cmd === "cancel_transfer") {
        return null;
      }
    });

    const { result } = renderHook(() => useTransfers());

    await waitFor(() => expect(progressHandlers.size).toBe(1));

    const started: TransferProgress = {
      transferId: "tx-1",
      fileName: "doc.pdf",
      direction: "download",
      status: "in_progress",
      bytes: 100,
      total: 1000,
    };

    act(() => emitTransferProgress(started));
    expect(result.current.transfers).toHaveLength(1);
    expect(result.current.activeCount).toBe(1);

    act(() => result.current.cancelTransfer("tx-1"));

    expect(result.current.transfers[0]?.status).toBe("cancelled");
    expect(result.current.activeCount).toBe(0);
  });
});
