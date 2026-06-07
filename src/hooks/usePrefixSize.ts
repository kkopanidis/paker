import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { calculatePrefixSize } from "@/lib/tauri";
import type { PrefixSizeProgress, PrefixSizeResult } from "@/types/s3";

interface ActiveState {
  prefix: string;
  loading: boolean;
  progress: PrefixSizeProgress | null;
  error: string | null;
}

export function usePrefixSize(connectionId: string | null, bucket: string | null) {
  const cacheRef = useRef(new Map<string, PrefixSizeResult>());
  const [cacheVersion, setCacheVersion] = useState(0);
  const [active, setActive] = useState<ActiveState | null>(null);
  const runIdRef = useRef(0);

  useEffect(() => {
    cacheRef.current.clear();
    setCacheVersion((v) => v + 1);
    setActive(null);
  }, [connectionId, bucket]);

  const getCached = useCallback(
    (prefix: string) => {
      void cacheVersion;
      return cacheRef.current.get(prefix) ?? null;
    },
    [cacheVersion]
  );

  const calculate = useCallback(
    async (prefix: string, options?: { force?: boolean }) => {
      if (!connectionId || !bucket) return null;

      const cached = cacheRef.current.get(prefix);
      if (cached && !options?.force) return cached;

      const runId = ++runIdRef.current;
      setActive({ prefix, loading: true, progress: null, error: null });

      let unlisten: (() => void) | undefined;

      try {
        unlisten = await listen<PrefixSizeProgress>("prefix-size-progress", (event) => {
          if (runId !== runIdRef.current) return;
          setActive((current) =>
            current?.prefix === prefix
              ? { ...current, progress: event.payload, error: event.payload.error ?? null }
              : current
          );
        });

        const result = await calculatePrefixSize(
          connectionId,
          bucket,
          prefix,
          options?.force
        );
        if (runId !== runIdRef.current) return null;

        cacheRef.current.set(prefix, result);
        setCacheVersion((v) => v + 1);
        setActive(null);
        return result;
      } catch (err) {
        if (runId !== runIdRef.current) return null;
        const message = err instanceof Error ? err.message : String(err);
        setActive({ prefix, loading: false, progress: null, error: message });
        return null;
      } finally {
        unlisten?.();
      }
    },
    [connectionId, bucket]
  );

  const getActiveFor = useCallback(
    (prefix: string) => {
      if (!active || active.prefix !== prefix) {
        return { loading: false, progress: null, error: null };
      }
      return {
        loading: active.loading,
        progress: active.progress,
        error: active.error,
      };
    },
    [active]
  );

  const seedCache = useCallback((prefix: string, result: PrefixSizeResult) => {
    cacheRef.current.set(prefix, result);
    setCacheVersion((v) => v + 1);
  }, []);

  return { getCached, getActiveFor, calculate, seedCache };
}
