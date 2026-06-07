import { useCallback, useEffect, useState } from "react";
import { getBookmarks, getFullUiState, setPanelLayout, setUiPreferences } from "@/lib/tauri";
import type { PanelLayout, PanelLayoutMode, PrefixBookmark, UiPreferences } from "@/types/ui";

const DEFAULT_PREFERENCES: UiPreferences = {
  localPanelOpen: false,
  detailsPaneOpen: true,
  connectionsCollapsed: true,
  bucketsCollapsed: true,
};

export function useUiState() {
  const [ready, setReady] = useState(false);
  const [preferences, setPreferences] = useState<UiPreferences>(DEFAULT_PREFERENCES);
  const [panelLayoutThree, setPanelLayoutThree] = useState<PanelLayout | null>(null);
  const [panelLayoutFour, setPanelLayoutFour] = useState<PanelLayout | null>(null);
  const [bookmarksByConnection, setBookmarksByConnection] = useState<
    Record<string, PrefixBookmark[]>
  >({});

  useEffect(() => {
    let cancelled = false;

    void getFullUiState()
      .then((state) => {
        if (cancelled) return;
        setPreferences({ ...DEFAULT_PREFERENCES, ...state.preferences });
        setPanelLayoutThree(state.panelLayoutThree ?? null);
        setPanelLayoutFour(state.panelLayoutFour ?? null);
        setBookmarksByConnection(state.bookmarks ?? {});
        setReady(true);
      })
      .catch(() => {
        if (!cancelled) setReady(true);
      });

    return () => {
      cancelled = true;
    };
  }, []);

  const savePreferences = useCallback(async (next: UiPreferences) => {
    setPreferences(next);
    await setUiPreferences(next);
  }, []);

  const persistPanelLayout = useCallback(async (mode: PanelLayoutMode, layout: PanelLayout) => {
    if (mode === "three") setPanelLayoutThree(layout);
    else setPanelLayoutFour(layout);
    await setPanelLayout(mode, layout);
  }, []);

  const refreshBookmarks = useCallback(async (connectionId: string) => {
    const bookmarks = await getBookmarks(connectionId);
    setBookmarksByConnection((current) => ({ ...current, [connectionId]: bookmarks }));
    return bookmarks;
  }, []);

  const getLayoutForMode = useCallback(
    (localPanelOpen: boolean): PanelLayout | undefined => {
      const saved = localPanelOpen ? panelLayoutFour : panelLayoutThree;
      if (saved) return saved;
      return localPanelOpen
        ? { connections: 18, buckets: 14, local: 28, browser: 40 }
        : { connections: 22, buckets: 20, browser: 58 };
    },
    [panelLayoutThree, panelLayoutFour]
  );

  return {
    ready,
    preferences,
    savePreferences,
    persistPanelLayout,
    bookmarksByConnection,
    setBookmarksByConnection,
    refreshBookmarks,
    getLayoutForMode,
  };
}
