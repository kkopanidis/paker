import { afterEach, describe, expect, it, vi } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { BucketIndexMeta, BucketIndexProgress } from "@/types/s3";
import { useBucketIndex } from "./useBucketIndex";

type ProgressHandler = (event: { payload: BucketIndexProgress }) => void;

const progressHandlers = new Set<ProgressHandler>();

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(
    (event: string, handler: ProgressHandler): Promise<() => void> => {
      if (event === "bucket-index-progress") {
        progressHandlers.add(handler);
        return Promise.resolve(() => progressHandlers.delete(handler));
      }
      return Promise.resolve(() => undefined);
    }
  ),
}));

function emitBucketIndexProgress(payload: BucketIndexProgress) {
  for (const handler of progressHandlers) {
    handler({ payload });
  }
}

const connectionId = "conn-a";
const bucket = "bucket-a";

const idleMeta: BucketIndexMeta = {
  connectionId,
  bucket,
  status: "idle",
  objectCount: 0,
};

const completedMeta: BucketIndexMeta = {
  connectionId,
  bucket,
  status: "completed",
  objectCount: 42,
  completedAt: "2024-06-01T12:00:00Z",
};

afterEach(() => {
  clearMocks();
  progressHandlers.clear();
  vi.clearAllMocks();
});

describe("useBucketIndex", () => {
  it("updates progress state from bucket-index-progress events", async () => {
    mockIPC((cmd) => {
      if (cmd === "get_bucket_index_status") return idleMeta;
    });

    const { result } = renderHook(() => useBucketIndex(connectionId, bucket));

    await waitFor(() => expect(progressHandlers.size).toBe(1));
    await waitFor(() => expect(result.current.loadingStatus).toBe(false));

    act(() =>
      emitBucketIndexProgress({
        connectionId,
        bucket,
        objectCount: 10,
        status: "running",
        done: false,
      })
    );

    expect(result.current.progress?.objectCount).toBe(10);
    expect(result.current.progress?.status).toBe("running");
    expect(result.current.meta?.status).toBe("running");
    expect(result.current.isActive).toBe(true);
  });

  it("handles terminal completed and failed progress states", async () => {
    let statusCall = 0;

    mockIPC((cmd) => {
      if (cmd === "get_bucket_index_status") {
        statusCall += 1;
        return statusCall === 1 ? idleMeta : completedMeta;
      }
    });

    const { result } = renderHook(() => useBucketIndex(connectionId, bucket));

    await waitFor(() => expect(progressHandlers.size).toBe(1));

    act(() =>
      emitBucketIndexProgress({
        connectionId,
        bucket,
        objectCount: 42,
        status: "completed",
        done: true,
      })
    );

    await waitFor(() => {
      expect(result.current.meta?.status).toBe("completed");
    });
    expect(result.current.progress?.done).toBe(true);
    expect(result.current.isSearchable).toBe(true);

    act(() =>
      emitBucketIndexProgress({
        connectionId,
        bucket,
        objectCount: 0,
        status: "failed",
        done: true,
        error: "index build failed",
      })
    );

    expect(result.current.progress?.status).toBe("failed");
    expect(result.current.progress?.error).toBe("index build failed");
    expect(result.current.meta?.status).toBe("failed");
    expect(result.current.meta?.error).toBe("index build failed");
  });
});
