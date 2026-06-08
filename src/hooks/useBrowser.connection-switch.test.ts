import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import type { S3Connection } from "@/types/connection";
import type { ListObjectsResponse } from "@/types/s3";
import { listObjects, readListCache } from "@/lib/tauri";
import { useBrowser } from "./useBrowser";

vi.mock("sonner", () => ({
  toast: {
    error: vi.fn(),
    success: vi.fn(),
    message: vi.fn(),
  },
}));

vi.mock("@/lib/tauri", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/tauri")>();
  return {
    ...actual,
    listObjects: vi.fn(),
    readListCache: vi.fn(),
  };
});

const emptyListing: ListObjectsResponse = {
  objects: [],
  commonPrefixes: [],
  continuationToken: null,
  isTruncated: false,
  fromCache: false,
  fetchedAt: "1700000000",
};

const connA: S3Connection = {
  id: "conn-a",
  name: "Connection A",
  region: "us-east-1",
  accessKeyId: "key-a",
  forcePathStyle: false,
  defaultBucket: "bucket-a",
};

const connB: S3Connection = {
  id: "conn-b",
  name: "Connection B",
  region: "us-east-1",
  accessKeyId: "key-b",
  forcePathStyle: false,
  defaultBucket: "bucket-b",
};

beforeEach(() => {
  vi.mocked(readListCache).mockResolvedValue(null);
  vi.mocked(listObjects).mockResolvedValue(emptyListing);
});

afterEach(() => {
  vi.clearAllMocks();
});

describe("useBrowser connection switch", () => {
  it("does not list objects with the previous connection bucket after switching", async () => {
    const { result, rerender } = renderHook(
      ({ connection }: { connection: S3Connection }) => useBrowser(connection),
      { initialProps: { connection: connA } }
    );

    await waitFor(() => {
      expect(result.current.selectedBucket).toBe("bucket-a");
    });

    await waitFor(() => {
      expect(listObjects).toHaveBeenCalledWith(
        "conn-a",
        "bucket-a",
        "",
        undefined,
        false
      );
    });

    vi.mocked(listObjects).mockClear();

    rerender({ connection: connB });

    await waitFor(() => {
      expect(result.current.selectedBucket).toBe("bucket-b");
    });

    await waitFor(() => {
      expect(listObjects).toHaveBeenCalledWith(
        "conn-b",
        "bucket-b",
        "",
        undefined,
        false
      );
    });

    const staleCalls = vi.mocked(listObjects).mock.calls.filter(
      ([connectionId, bucket]) => connectionId === "conn-b" && bucket === "bucket-a"
    );
    expect(staleCalls).toHaveLength(0);
  });
});
