import { afterEach, describe, expect, it } from "vitest";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { invokeSafe, parseStructuredError } from "./tauri";

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
