import { useCallback, useEffect, useMemo, useState } from "react";
import { toast } from "sonner";
import {
  getHomeDir,
  getLastLocalDir,
  getParentPath,
  listLocalDir,
  pickLocalFolder,
  setLastLocalDir,
} from "@/lib/tauri";
import type { LocalEntry } from "@/types/local";

function pathSegments(path: string): { label: string; path: string }[] {
  if (!path) return [];
  const parts = path.replace(/\\/g, "/").split("/").filter(Boolean);
  return parts.map((part, index) => ({
    label: part,
    path: `/${parts.slice(0, index + 1).join("/")}`,
  }));
}

export function useLocalBrowser(connectionId: string | null) {
  const [cwd, setCwd] = useState<string>("");
  const [entries, setEntries] = useState<LocalEntry[]>([]);
  const [selectedPaths, setSelectedPaths] = useState<Set<string>>(new Set());
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState(false);

  const loadDir = useCallback(async (path: string) => {
    setLoading(true);
    try {
      const result = await listLocalDir(path);
      const sorted = [...result].sort((a, b) => {
        if (a.isDir !== b.isDir) return a.isDir ? -1 : 1;
        return a.name.localeCompare(b.name);
      });
      setEntries(sorted);
      setCwd(path);
      setSelectedPaths(new Set());
    } catch (error) {
      toast.error("Failed to list directory", {
        description: error instanceof Error ? error.message : String(error),
      });
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    let cancelled = false;

    async function init() {
      try {
        let initialPath: string | null = null;
        if (connectionId) {
          initialPath = await getLastLocalDir(connectionId);
        }
        if (!initialPath) {
          initialPath = await getHomeDir();
        }
        if (!cancelled) {
          await loadDir(initialPath);
        }
      } catch {
        if (!cancelled) {
          try {
            const home = await getHomeDir();
            if (!cancelled) await loadDir(home);
          } catch (error) {
            toast.error("Failed to load home directory", {
              description: error instanceof Error ? error.message : String(error),
            });
          }
        }
      }
    }

    void init();
    return () => {
      cancelled = true;
    };
  }, [connectionId, loadDir]);

  const navigateUp = useCallback(async () => {
    if (!cwd) return;
    try {
      const parent = await getParentPath(cwd);
      if (parent) await loadDir(parent);
    } catch (error) {
      toast.error("Failed to navigate up", {
        description: error instanceof Error ? error.message : String(error),
      });
    }
  }, [cwd, loadDir]);

  const navigateInto = useCallback(
    async (entry: LocalEntry) => {
      if (!entry.isDir) return;
      await loadDir(entry.path);
      if (connectionId) {
        try {
          await setLastLocalDir(connectionId, entry.path);
        } catch {
          // non-critical persistence failure
        }
      }
    },
    [loadDir, connectionId]
  );

  const selectPaths = useCallback((paths: string[], selected: boolean) => {
    setSelectedPaths((current) => {
      const next = new Set(current);
      for (const p of paths) {
        if (selected) next.add(p);
        else next.delete(p);
      }
      return next;
    });
  }, []);

  const selectAll = useCallback(() => {
    setSelectedPaths(new Set(entries.map((e) => e.path)));
  }, [entries]);

  const clearSelection = useCallback(() => {
    setSelectedPaths(new Set());
  }, []);

  const pickFolder = useCallback(async () => {
    setBusy(true);
    try {
      const picked = await pickLocalFolder();
      if (picked) {
        await loadDir(picked);
        if (connectionId) {
          try {
            await setLastLocalDir(connectionId, picked);
          } catch {
            // non-critical
          }
        }
      }
    } catch (error) {
      toast.error("Failed to pick folder", {
        description: error instanceof Error ? error.message : String(error),
      });
    } finally {
      setBusy(false);
    }
  }, [loadDir, connectionId]);

  const refresh = useCallback(async () => {
    if (cwd) await loadDir(cwd);
  }, [cwd, loadDir]);

  const breadcrumbs = useMemo(() => pathSegments(cwd), [cwd]);

  const selectedEntries = useMemo(
    () => entries.filter((e) => selectedPaths.has(e.path)),
    [entries, selectedPaths]
  );

  return {
    cwd,
    entries,
    selectedPaths,
    selectedEntries,
    loading,
    busy,
    breadcrumbs,
    loadDir,
    navigateUp,
    navigateInto,
    selectPaths,
    selectAll,
    clearSelection,
    pickFolder,
    refresh,
  };
}
