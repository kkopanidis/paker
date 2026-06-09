import { randomFillSync } from "crypto";
import { cleanup } from "@testing-library/react";
import { afterEach, beforeAll, vi } from "vitest";

// jsdom does not provide WebCrypto; Tauri mockIPC expects it.
beforeAll(() => {
  Object.defineProperty(window, "crypto", {
    value: {
      getRandomValues: (buffer: Uint8Array) => randomFillSync(buffer),
    },
  });

  // Radix UI primitives expect pointer capture and layout APIs in jsdom.
  HTMLElement.prototype.hasPointerCapture = vi.fn().mockReturnValue(false);
  HTMLElement.prototype.setPointerCapture = vi.fn();
  HTMLElement.prototype.releasePointerCapture = vi.fn();
  HTMLElement.prototype.scrollIntoView = vi.fn();

  class ResizeObserverMock {
    observe = vi.fn();
    unobserve = vi.fn();
    disconnect = vi.fn();
  }

  global.ResizeObserver = ResizeObserverMock as unknown as typeof ResizeObserver;
});

afterEach(() => {
  cleanup();
});
