import { afterEach, describe, expect, it, vi } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { toast } from "sonner";
import { useLocalBrowser } from "./useLocalBrowser";

vi.mock("sonner", () => ({
  toast: {
    error: vi.fn(),
    success: vi.fn(),
  },
}));

afterEach(() => {
  clearMocks();
  vi.clearAllMocks();
});

describe("useLocalBrowser", () => {
  it("surfaces scoped path errors from list_local_dir", async () => {
    mockIPC((cmd) => {
      if (cmd === "get_last_local_dir") {
        return "/Users/test/projects";
      }
      if (cmd === "list_local_dir") {
        throw {
          code: "path_not_allowed",
          message: "Path is not allowed",
          userAction: "Use the file picker or home directory.",
        };
      }
    });

    const { result } = renderHook(() => useLocalBrowser("conn-1"));

    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(toast.error).toHaveBeenCalledWith("Failed to list directory", {
      description: "Path is not allowed Use the file picker or home directory.",
    });
    expect(result.current.entries).toEqual([]);
  });
});
