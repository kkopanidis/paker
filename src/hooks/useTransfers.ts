import { useCallback, useEffect, useMemo, useState } from "react";
import { listen } from "@tauri-apps/api/event";
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

  const clearCompleted = useCallback(() => {
    setTransfers((current) =>
      current.filter(
        (t) => t.status !== "completed" && t.status !== "failed"
      )
    );
  }, []);

  const activeCount = useMemo(
    () =>
      transfers.filter(
        (t) => t.status === "started" || t.status === "in_progress"
      ).length,
    [transfers]
  );

  return {
    transfers,
    activeCount,
    clearCompleted,
  };
}
