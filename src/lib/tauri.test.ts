import { afterEach, describe, expect, it } from "vitest";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { formatIpcError, invokeSafe, parseStructuredError } from "./tauri";

afterEach(() => {
  clearMocks();
});

describe("parseStructuredError", () => {
  it("parses structured PakerError objects", () => {
    expect(
      parseStructuredError({
        code: "connection_not_found",
        message: "Connection not found",
        userAction: "Select a different profile.",
      })
    ).toEqual({
      code: "connection_not_found",
      message: "Connection not found",
      userAction: "Select a different profile.",
    });
  });

  it("parses JSON-encoded error strings", () => {
    expect(
      parseStructuredError(
        JSON.stringify({
          code: "s3_error",
          message: "Access denied",
        })
      )
    ).toEqual({
      code: "s3_error",
      message: "Access denied",
      userAction: undefined,
    });
  });
});

describe("formatIpcError", () => {
  it("includes userAction when present", () => {
    expect(
      formatIpcError({
        code: "pathNotAllowed",
        message: "Path is not allowed",
        userAction: "Use the file picker.",
      })
    ).toBe("Path is not allowed Use the file picker.");
  });

  it("returns message only when userAction is absent", () => {
    expect(
      formatIpcError({
        code: "invalid_input",
        message: "Invalid path",
      })
    ).toBe("Invalid path");
  });

  it("formats plain Error instances", () => {
    expect(formatIpcError(new Error("network timeout"))).toBe("network timeout");
  });
});

describe("parseStructuredError edge cases", () => {
  it("falls back for unknown object shapes", () => {
    expect(parseStructuredError({ foo: "bar" })).toEqual({
      code: "unknown",
      message: "[object Object]",
    });
  });

  it("parses Error with JSON message payload", () => {
    expect(
      parseStructuredError(
        new Error(
          JSON.stringify({
            code: "path_not_allowed",
            message: "Path is not allowed",
          })
        )
      )
    ).toEqual({
      code: "path_not_allowed",
      message: "Path is not allowed",
      userAction: undefined,
    });
  });
});

describe("invokeSafe", () => {
  it("rethrows structured PakerIpcError on invoke rejection", async () => {
    mockIPC((cmd) => {
      if (cmd === "test_connection") {
        throw {
          code: "connection_failed",
          message: "Could not reach endpoint",
          userAction: "Check the endpoint URL.",
        };
      }
    });

    await expect(invokeSafe("test_connection", { id: "conn-1" })).rejects.toEqual({
      code: "connection_failed",
      message: "Could not reach endpoint",
      userAction: "Check the endpoint URL.",
    });
  });
});
