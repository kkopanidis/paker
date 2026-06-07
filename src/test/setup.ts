import { randomFillSync } from "crypto";
import { beforeAll } from "vitest";

// jsdom does not provide WebCrypto; Tauri mockIPC expects it.
beforeAll(() => {
  Object.defineProperty(window, "crypto", {
    value: {
      getRandomValues: (buffer: Uint8Array) => randomFillSync(buffer),
    },
  });
});
