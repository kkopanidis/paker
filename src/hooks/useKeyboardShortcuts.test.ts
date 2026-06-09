import { afterEach, describe, expect, it, vi } from "vitest";
import { renderHook } from "@testing-library/react";
import { useKeyboardShortcuts } from "./useKeyboardShortcuts";

function dispatchKey(
  key: string,
  options: KeyboardEventInit & { target?: EventTarget | null } = {}
) {
  const { target = document.body, ...init } = options;
  const event = new KeyboardEvent("keydown", {
    bubbles: true,
    cancelable: true,
    ...init,
    key,
  });
  Object.defineProperty(event, "target", { value: target });
  window.dispatchEvent(event);
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe("useKeyboardShortcuts", () => {
  it("calls onRefresh on F5 when focus is not in an input", () => {
    const onRefresh = vi.fn();
    const onDelete = vi.fn();

    renderHook(() =>
      useKeyboardShortcuts({
        onRefresh,
        onDelete,
      })
    );

    dispatchKey("F5");

    expect(onRefresh).toHaveBeenCalledTimes(1);
    expect(onDelete).not.toHaveBeenCalled();
  });

  it("calls onDelete on Delete key when focus is not in an input", () => {
    const onRefresh = vi.fn();
    const onDelete = vi.fn();

    renderHook(() =>
      useKeyboardShortcuts({
        onRefresh,
        onDelete,
      })
    );

    dispatchKey("Delete");

    expect(onDelete).toHaveBeenCalledTimes(1);
    expect(onRefresh).not.toHaveBeenCalled();
  });

  it("ignores shortcuts when focus is in an INPUT", () => {
    const onRefresh = vi.fn();
    const onDelete = vi.fn();
    const input = document.createElement("input");
    document.body.appendChild(input);

    renderHook(() =>
      useKeyboardShortcuts({
        onRefresh,
        onDelete,
      })
    );

    dispatchKey("F5", { target: input });
    dispatchKey("Delete", { target: input });

    expect(onRefresh).not.toHaveBeenCalled();
    expect(onDelete).not.toHaveBeenCalled();

    input.remove();
  });
});
