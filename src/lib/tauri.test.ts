import { afterEach, describe, expect, it } from "vitest";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { formatIpcError, invokeSafe, normalizeObjects, parseStructuredError } from "./tauri";
import type { ListObjectsResult } from "@/types/s3";

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

describe("normalizeObjects", () => {
  it("creates folder entries from common prefixes with marker lastModified", () => {
    const result: ListObjectsResult = {
      objects: [
        {
          key: "photos/",
          size: 0,
          lastModified: "2024-03-10T12:00:00Z",
        },
        {
          key: "photos/cat.jpg",
          size: 1024,
          lastModified: "2024-01-01T00:00:00Z",
        },
      ],
      commonPrefixes: ["photos/"],
      isTruncated: false,
    };

    const objects = normalizeObjects(result, "");
    const folder = objects.find((o) => o.isFolder);
    expect(folder?.name).toBe("photos");
    expect(folder?.key).toBe("photos/");
    expect(folder?.lastModified).toBe("2024-03-10T12:00:00Z");
  });

  it("uses prefixLastModified from bucket index when no folder marker", () => {
    const result: ListObjectsResult = {
      objects: [],
      commonPrefixes: ["docs/"],
      prefixLastModified: { "docs/": "2024-08-20T10:00:00Z" },
      isTruncated: false,
    };

    const objects = normalizeObjects(result, "");
    const folder = objects.find((o) => o.isFolder);
    expect(folder?.lastModified).toBe("2024-08-20T10:00:00Z");
  });

  it("returns an empty array for an empty listing", () => {
    const result: ListObjectsResult = {
      objects: [],
      commonPrefixes: [],
      isTruncated: false,
    };

    expect(normalizeObjects(result, "")).toEqual([]);
  });

  it("normalizes nested common prefixes under the current prefix", () => {
    const result: ListObjectsResult = {
      objects: [
        {
          key: "projects/alpha/readme.txt",
          size: 128,
          lastModified: "2024-04-01T00:00:00Z",
        },
      ],
      commonPrefixes: ["projects/alpha/", "projects/beta/"],
      isTruncated: false,
    };

    const objects = normalizeObjects(result, "projects/");
    expect(objects.map((o) => ({ name: o.name, isFolder: o.isFolder }))).toEqual([
      { name: "alpha", isFolder: true },
      { name: "beta", isFolder: true },
      { name: "alpha/readme.txt", isFolder: false },
    ]);
  });

  it("preserves isTruncated metadata on the source listing", () => {
    const result: ListObjectsResult = {
      objects: [{ key: "page-1.txt", size: 1, lastModified: "2024-01-01T00:00:00Z" }],
      commonPrefixes: [],
      continuationToken: "next-page",
      isTruncated: true,
    };

    expect(result.isTruncated).toBe(true);
    expect(result.continuationToken).toBe("next-page");
    expect(normalizeObjects(result, "").map((o) => o.key)).toEqual(["page-1.txt"]);
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
