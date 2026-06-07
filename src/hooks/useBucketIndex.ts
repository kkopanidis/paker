import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  cancelBucketIndex,
  getBucketIndexStatus,
  pauseBucketIndex,
  resumeBucketIndex,
  startBucketIndex,
} from "@/lib/tauri";
import type { BucketIndexMeta, BucketIndexProgress } from "@/types/s3";

export function useBucketIndex(connectionId: string | null, bucket: string | null) {
  const [meta, setMeta] = useState<BucketIndexMeta | null>(null);
  const [progress, setProgress] = useState<BucketIndexProgress | null>(null);
  const [loadingStatus, setLoadingStatus] = useState(false);

  const refreshStatus = useCallback(async () => {
    if (!connectionId || !bucket) {
      setMeta(null);
      return;
    }

    setLoadingStatus(true);
    try {
      const status = await getBucketIndexStatus(connectionId, bucket);
      setMeta(status);
    } finally {
      setLoadingStatus(false);
    }
  }, [connectionId, bucket]);

  useEffect(() => {
    void refreshStatus();
  }, [refreshStatus]);

  useEffect(() => {
    if (!connectionId || !bucket) return;

    let unlisten: (() => void) | undefined;

    void listen<BucketIndexProgress>("bucket-index-progress", (event) => {
      const payload = event.payload;
      if (payload.connectionId !== connectionId || payload.bucket !== bucket) return;

      setProgress(payload);
      setMeta((current) => ({
        connectionId,
        bucket,
        status: payload.status,
        objectCount: payload.objectCount,
        startedAt: current?.startedAt,
        completedAt: payload.done ? new Date().toISOString() : current?.completedAt,
        error: payload.error,
      }));

      if (payload.done) {
        void refreshStatus();
      }
    }).then((fn) => {
      unlisten = fn;
    });

    return () => {
      unlisten?.();
    };
  }, [connectionId, bucket, refreshStatus]);

  const start = useCallback(
    async (rebuild = true) => {
      if (!connectionId || !bucket) return;
      setProgress(null);
      await startBucketIndex(connectionId, bucket, rebuild);
      await refreshStatus();
    },
    [connectionId, bucket, refreshStatus]
  );

  const pause = useCallback(async () => {
    if (!connectionId || !bucket) return;
    await pauseBucketIndex(connectionId, bucket);
    await refreshStatus();
  }, [connectionId, bucket, refreshStatus]);

  const resume = useCallback(async () => {
    if (!connectionId || !bucket) return;
    await resumeBucketIndex(connectionId, bucket);
    await refreshStatus();
  }, [connectionId, bucket, refreshStatus]);

  const cancel = useCallback(async () => {
    if (!connectionId || !bucket) return;
    await cancelBucketIndex(connectionId, bucket);
    await refreshStatus();
  }, [connectionId, bucket, refreshStatus]);

  const isActive =
    meta?.status === "running" || meta?.status === "paused" || progress?.status === "running";

  const isSearchable =
    meta?.status === "completed" || meta?.status === "stale";

  return {
    meta,
    progress,
    loadingStatus,
    isActive,
    isSearchable,
    refreshStatus,
    start,
    pause,
    resume,
    cancel,
  };
}
