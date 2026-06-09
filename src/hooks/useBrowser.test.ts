import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";
import { toast } from "sonner";
import type { ListObjectsResponse } from "@/types/s3";
import { connA, emptyListing } from "@/test/fixtures";
import {
  deleteObjects,
  listObjects,
  readListCache,
} from "@/lib/tauri";
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
    deleteObjects: vi.fn(),
  };
});

beforeEach(() => {
  vi.mocked(readListCache).mockResolvedValue(null);
  vi.mocked(listObjects).mockResolvedValue(emptyListing);
  vi.mocked(deleteObjects).mockResolvedValue(undefined);
});

afterEach(() => {
  vi.clearAllMocks();
});

describe("useBrowser", () => {
  it("appends objects when loadMoreObjects uses continuationToken", async () => {
    const page1: ListObjectsResponse = {
      objects: [{ key: "alpha.txt", size: 10, lastModified: "2024-01-01T00:00:00Z" }],
      commonPrefixes: [],
      continuationToken: "page-2",
      isTruncated: true,
      fromCache: false,
      fetchedAt: "1700000001",
    };
    const page2: ListObjectsResponse = {
      objects: [{ key: "beta.txt", size: 20, lastModified: "2024-01-02T00:00:00Z" }],
      commonPrefixes: [],
      continuationToken: undefined,
      isTruncated: false,
      fromCache: false,
      fetchedAt: "1700000002",
    };

    vi.mocked(listObjects).mockImplementation(async (_id, _bucket, _prefix, token) => {
      if (token === "page-2") return page2;
      return page1;
    });

    const { result } = renderHook(() => useBrowser(connA));

    await waitFor(() => {
      expect(result.current.objects.map((o) => o.key)).toEqual(["alpha.txt"]);
    });
    expect(result.current.hasMore).toBe(true);

    await act(async () => {
      await result.current.loadMoreObjects();
    });

    await waitFor(() => {
      expect(result.current.objects.map((o) => o.key)).toEqual(["alpha.txt", "beta.txt"]);
    });
    expect(result.current.hasMore).toBe(false);
    expect(listObjects).toHaveBeenCalledWith("conn-a", "bucket-a", "", "page-2");
  });

  it("shows cached listing first then refreshes from listObjects", async () => {
    const cachedListing: ListObjectsResponse = {
      objects: [{ key: "cached.txt", size: 1, lastModified: "2024-01-01T00:00:00Z" }],
      commonPrefixes: [],
      continuationToken: undefined,
      isTruncated: false,
      fromCache: true,
      fetchedAt: "cached-at",
    };
    const freshListing: ListObjectsResponse = {
      objects: [{ key: "fresh.txt", size: 2, lastModified: "2024-02-01T00:00:00Z" }],
      commonPrefixes: [],
      continuationToken: undefined,
      isTruncated: false,
      fromCache: false,
      fetchedAt: "fresh-at",
    };

    let resolveList: (value: ListObjectsResponse) => void = () => undefined;
    const listPromise = new Promise<ListObjectsResponse>((resolve) => {
      resolveList = resolve;
    });

    vi.mocked(readListCache).mockResolvedValue({
      result: cachedListing,
      fetchedAt: "cached-at",
    });
    vi.mocked(listObjects).mockReturnValue(listPromise);

    const { result } = renderHook(() => useBrowser(connA));

    await waitFor(() => {
      expect(result.current.objects.map((o) => o.key)).toEqual(["cached.txt"]);
    });
    expect(result.current.objectsStale).toBe(true);
    expect(result.current.objectsFetchedAt).toBe("cached-at");

    await act(async () => {
      resolveList(freshListing);
    });

    await waitFor(() => {
      expect(result.current.objects.map((o) => o.key)).toEqual(["fresh.txt"]);
    });
    expect(result.current.objectsStale).toBe(false);
    expect(result.current.objectsFetchedAt).toBe("fresh-at");
    expect(readListCache).toHaveBeenCalledWith("conn-a", "bucket-a", "");
    expect(listObjects).toHaveBeenCalledWith("conn-a", "bucket-a", "", undefined, false);
  });

  it("delete success path calls deleteObjects and refreshes listing", async () => {
    const listing: ListObjectsResponse = {
      objects: [{ key: "remove-me.txt", size: 5, lastModified: "2024-01-01T00:00:00Z" }],
      commonPrefixes: [],
      continuationToken: undefined,
      isTruncated: false,
      fromCache: false,
      fetchedAt: "1700000000",
    };

    vi.mocked(listObjects).mockResolvedValue(listing);

    const { result } = renderHook(() => useBrowser(connA));

    await waitFor(() => {
      expect(result.current.objects).toHaveLength(1);
    });

    vi.mocked(listObjects).mockClear();

    await act(async () => {
      await result.current.removeObjects(result.current.objects);
    });

    expect(deleteObjects).toHaveBeenCalledWith("conn-a", "bucket-a", ["remove-me.txt"]);
    expect(toast.success).toHaveBeenCalledWith("Delete completed");
    expect(listObjects).toHaveBeenCalledWith("conn-a", "bucket-a", "", undefined, true);
  });

  it("shows toast when listObjects fails without cache", async () => {
    vi.mocked(listObjects).mockRejectedValue({
      code: "s3_error",
      message: "Access denied",
    });

    renderHook(() => useBrowser(connA));

    await waitFor(() => {
      expect(toast.error).toHaveBeenCalledWith("Failed to list objects", {
        description: "Access denied",
      });
    });
  });
});
