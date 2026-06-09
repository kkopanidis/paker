import { afterEach, describe, expect, it, vi } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { PrefixSizeProgress, PrefixSizeResult } from "@/types/s3";
import { usePrefixSize } from "./usePrefixSize";

type ProgressHandler = (event: { payload: PrefixSizeProgress }) => void;

const progressHandlers = new Set<ProgressHandler>();

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(
    (event: string, handler: ProgressHandler): Promise<() => void> => {
      if (event === "prefix-size-progress") {
        progressHandlers.add(handler);
        return Promise.resolve(() => progressHandlers.delete(handler));
      }
      return Promise.resolve(() => undefined);
    }
  ),
}));

function emitPrefixSizeProgress(payload: PrefixSizeProgress) {
  for (const handler of progressHandlers) {
    handler({ payload });
  }
}

const connectionId = "conn-a";
const bucket = "bucket-a";
const prefix = "docs/";

const finalResult: PrefixSizeResult = {
  prefix,
  objectCount: 3,
  totalBytes: 9000,
};

afterEach(() => {
  clearMocks();
  progressHandlers.clear();
  vi.clearAllMocks();
});

describe("usePrefixSize", () => {
  it("updates active progress from prefix-size-progress events", async () => {
    let resolveCalc: (value: PrefixSizeResult) => void = () => undefined;
    const calcPromise = new Promise<PrefixSizeResult>((resolve) => {
      resolveCalc = resolve;
    });

    mockIPC((cmd) => {
      if (cmd === "calculate_prefix_size") return calcPromise;
    });

    const { result } = renderHook(() => usePrefixSize(connectionId, bucket));

    let calculatePromise: Promise<PrefixSizeResult | null>;
    act(() => {
      calculatePromise = result.current.calculate(prefix);
    });

    await waitFor(() => {
      expect(result.current.getActiveFor(prefix).loading).toBe(true);
    });

    act(() =>
      emitPrefixSizeProgress({
        prefix,
        objectCount: 2,
        totalBytes: 5000,
        done: false,
      })
    );

    expect(result.current.getActiveFor(prefix).progress?.objectCount).toBe(2);

    await act(async () => {
      resolveCalc(finalResult);
      await calculatePromise!;
    });

    expect(result.current.getCached(prefix)).toEqual(finalResult);
    expect(result.current.getActiveFor(prefix)).toEqual({
      loading: false,
      progress: null,
      error: null,
    });
  });

  it("clears loading state after calculation completes", async () => {
    mockIPC((cmd) => {
      if (cmd === "calculate_prefix_size") return finalResult;
    });

    const { result } = renderHook(() => usePrefixSize(connectionId, bucket));

    await act(async () => {
      await result.current.calculate(prefix);
    });

    expect(result.current.getActiveFor(prefix).loading).toBe(false);
    expect(result.current.getCached(prefix)).toEqual(finalResult);
  });
});
