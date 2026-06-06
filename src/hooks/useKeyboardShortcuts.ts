import { useEffect } from "react";

interface KeyboardShortcuts {
  onRefresh?: () => void;
  onDelete?: () => void;
  onUpload?: () => void;
  onOpenSelected?: () => void;
  onSelectAll?: () => void;
  onDownload?: () => void;
  disabled?: boolean;
}

export function useKeyboardShortcuts({
  onRefresh,
  onDelete,
  onUpload,
  onOpenSelected,
  onSelectAll,
  onDownload,
  disabled,
}: KeyboardShortcuts) {
  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      if (disabled) return;
      const target = event.target as HTMLElement | null;
      if (
        target?.tagName === "INPUT" ||
        target?.tagName === "TEXTAREA" ||
        target?.isContentEditable
      ) {
        return;
      }

      if (event.key === "F5") {
        event.preventDefault();
        onRefresh?.();
      } else if (event.key === "Delete" || event.key === "Backspace") {
        if (event.metaKey || event.ctrlKey) return;
        event.preventDefault();
        onDelete?.();
      } else if (event.key === "u" && (event.metaKey || event.ctrlKey)) {
        event.preventDefault();
        onUpload?.();
      } else if (event.key === "Enter") {
        event.preventDefault();
        onOpenSelected?.();
      } else if (event.key === "a" && (event.metaKey || event.ctrlKey)) {
        event.preventDefault();
        onSelectAll?.();
      } else if (event.key === "d" && (event.metaKey || event.ctrlKey)) {
        event.preventDefault();
        onDownload?.();
      }
    };

    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [disabled, onRefresh, onDelete, onUpload, onOpenSelected, onSelectAll, onDownload]);
}
