import { afterEach, describe, expect, it } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { PrefixBookmark } from "@/types/ui";
import { addBookmark, removeBookmark } from "@/lib/tauri";
import { useUiState } from "./useUiState";

const connectionId = "conn-a";

const initialBookmark: PrefixBookmark = {
  id: "bm-1",
  label: "Docs",
  bucket: "bucket-a",
  prefix: "docs/",
};

afterEach(() => {
  clearMocks();
});

describe("useUiState", () => {
  it("loads bookmarks from get_full_ui_state via IPC", async () => {
    mockIPC((cmd) => {
      if (cmd === "get_full_ui_state") {
        return {
          lastLocalDir: {},
          maxConcurrentTransfers: 3,
          lastNav: {},
          bookmarks: { [connectionId]: [initialBookmark] },
          preferences: {
            localPanelOpen: false,
            detailsPaneOpen: true,
            connectionsCollapsed: true,
            bucketsCollapsed: true,
            checkForUpdates: true,
          },
        };
      }
    });

    const { result } = renderHook(() => useUiState());

    await waitFor(() => expect(result.current.ready).toBe(true));

    expect(result.current.bookmarksByConnection[connectionId]).toEqual([initialBookmark]);
  });

  it("add and remove bookmark round-trip via refreshBookmarks", async () => {
    const bookmarks = new Map<string, PrefixBookmark[]>([[connectionId, [initialBookmark]]]);

    mockIPC((cmd, args) => {
      if (cmd === "get_full_ui_state") {
        return {
          lastLocalDir: {},
          maxConcurrentTransfers: 3,
          lastNav: {},
          bookmarks: Object.fromEntries(bookmarks),
          preferences: {
            localPanelOpen: false,
            detailsPaneOpen: true,
            connectionsCollapsed: true,
            bucketsCollapsed: true,
            checkForUpdates: true,
          },
        };
      }
      if (cmd === "get_bookmarks") {
        return bookmarks.get(args?.connectionId as string) ?? [];
      }
      if (cmd === "add_bookmark") {
        const { connectionId: connId, bookmark } = args as {
          connectionId: string;
          bookmark: PrefixBookmark;
        };
        const current = bookmarks.get(connId) ?? [];
        bookmarks.set(connId, [...current, bookmark]);
        return null;
      }
      if (cmd === "remove_bookmark") {
        const { connectionId: connId, bookmarkId } = args as {
          connectionId: string;
          bookmarkId: string;
        };
        const current = bookmarks.get(connId) ?? [];
        bookmarks.set(
          connId,
          current.filter((bookmark) => bookmark.id !== bookmarkId)
        );
        return null;
      }
    });

    const { result } = renderHook(() => useUiState());

    await waitFor(() => expect(result.current.ready).toBe(true));
    expect(result.current.bookmarksByConnection[connectionId]).toHaveLength(1);

    const added: PrefixBookmark = {
      id: "bm-2",
      label: "Photos",
      bucket: "bucket-a",
      prefix: "photos/",
    };

    await act(async () => {
      await addBookmark(connectionId, added);
      await result.current.refreshBookmarks(connectionId);
    });

    expect(result.current.bookmarksByConnection[connectionId]).toEqual([
      initialBookmark,
      added,
    ]);

    await act(async () => {
      await removeBookmark(connectionId, "bm-1");
      await result.current.refreshBookmarks(connectionId);
    });

    expect(result.current.bookmarksByConnection[connectionId]).toEqual([added]);
  });
});
