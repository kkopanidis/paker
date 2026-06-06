import { useCallback, useEffect, useMemo, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  cancelTransfer as tauriCancelTransfer,
  pauseTransfer as tauriPauseTransfer,
  resumeTransfer as tauriResumeTransfer,
} from "@/lib/tauri";
import type { TransferProgress } from "@/types/s3";

export function useTransfers() {
  const [transfers, setTransfers] = useState<TransferProgress[]>([]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;

    void listen<TransferProgress>("transfer-progress", (event) => {
      const progress = event.payload;
      setTransfers((current) => {
        const index = current.findIndex((t) => t.transferId === progress.transferId);
        if (index === -1) return [...current, progress];
        const next = [...current];
        next[index] = progress;
        return next;
      });
    }).then((fn) => {
      unlisten = fn;
    });

    return () => {
      unlisten?.();
    };
  }, []);

  const optimisticUpdate = useCallback(
    (id: string, status: TransferProgress["status"]) => {
      setTransfers((current) =>
        current.map((t) => (t.transferId === id ? { ...t, status } : t))
      );
    },
    []
  );

  const cancelTransfer = useCallback(
    (id: string) => {
      optimisticUpdate(id, "cancelled");
      void tauriCancelTransfer(id);
    },
    [optimisticUpdate]
  );

  const pauseTransfer = useCallback(
    (id: string) => {
      optimisticUpdate(id, "paused");
      void tauriPauseTransfer(id);
    },
    [optimisticUpdate]
  );

  const resumeTransfer = useCallback(
    (id: string) => {
      optimisticUpdate(id, "in_progress");
      void tauriResumeTransfer(id);
    },
    [optimisticUpdate]
  );

  const clearCompleted = useCallback(() => {
    setTransfers((current) =>
      current.filter(
        (t) =>
          t.status !== "completed" &&
          t.status !== "failed" &&
          t.status !== "cancelled"
      )
    );
  }, []);

  const activeCount = useMemo(
    () =>
      transfers.filter(
        (t) =>
          t.status === "started" ||
          t.status === "in_progress" ||
          t.status === "paused"
      ).length,
    [transfers]
  );

  return {
    transfers,
    activeCount,
    clearCompleted,
    cancelTransfer,
    pauseTransfer,
    resumeTransfer,
  };
}
