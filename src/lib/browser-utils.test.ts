import { describe, expect, it } from "vitest";
import { filterObjects, looksLikePath, matchesTypeFilter, resolveDeleteTargets } from "./browser-utils";
import type { S3Object } from "@/types/s3";

const folder: S3Object = {
  key: "docs/",
  name: "docs",
  isFolder: true,
  size: 0,
};

const file: S3Object = {
  key: "docs/readme.txt",
  name: "readme.txt",
  isFolder: false,
  size: 1024,
  storageClass: "STANDARD",
};

const glacierFile: S3Object = {
  key: "archive/old.zip",
  name: "old.zip",
  isFolder: false,
  size: 4096,
  storageClass: "GLACIER",
};

const deepArchiveFile: S3Object = {
  key: "archive/deep.bin",
  name: "deep.bin",
  isFolder: false,
  size: 8192,
  storageClass: "DEEP_ARCHIVE",
};

const sampleObjects: S3Object[] = [folder, file, glacierFile, deepArchiveFile];

describe("matchesTypeFilter", () => {
  it("matches folders only", () => {
    expect(matchesTypeFilter(folder, "folders")).toBe(true);
    expect(matchesTypeFilter(file, "folders")).toBe(false);
  });

  it("matches files only", () => {
    expect(matchesTypeFilter(file, "files")).toBe(true);
    expect(matchesTypeFilter(folder, "files")).toBe(false);
  });

  it("matches glacier storage classes case-insensitively", () => {
    expect(matchesTypeFilter(glacierFile, "glacier")).toBe(true);
    expect(
      matchesTypeFilter(
        { ...glacierFile, storageClass: "glacier_ir" },
        "glacier"
      )
    ).toBe(true);
    expect(matchesTypeFilter(file, "glacier")).toBe(false);
    expect(matchesTypeFilter(deepArchiveFile, "glacier")).toBe(false);
  });

  it("treats missing storageClass as non-glacier", () => {
    expect(matchesTypeFilter({ ...file, storageClass: undefined }, "glacier")).toBe(
      false
    );
  });

  it("returns true for all type filter regardless of object", () => {
    expect(matchesTypeFilter(folder, "all")).toBe(true);
    expect(matchesTypeFilter(glacierFile, "all")).toBe(true);
  });
});

describe("filterObjects", () => {
  it("returns all objects when no filters are applied", () => {
    expect(filterObjects(sampleObjects)).toEqual(sampleObjects);
    expect(filterObjects(sampleObjects, "", "all")).toEqual(sampleObjects);
    expect(filterObjects(sampleObjects, undefined, undefined)).toEqual(sampleObjects);
  });

  it("filters by name substring case-insensitively", () => {
    expect(filterObjects(sampleObjects, "readme")).toEqual([file]);
    expect(filterObjects(sampleObjects, "README")).toEqual([file]);
    expect(filterObjects(sampleObjects, "old")).toEqual([glacierFile]);
    expect(filterObjects(sampleObjects, ".zip")).toEqual([glacierFile]);
  });

  it("ignores whitespace-only filter text", () => {
    expect(filterObjects(sampleObjects, "   ")).toEqual(sampleObjects);
  });

  it("filters by type only", () => {
    expect(filterObjects(sampleObjects, undefined, "folders")).toEqual([folder]);
    expect(filterObjects(sampleObjects, undefined, "files")).toEqual([
      file,
      glacierFile,
      deepArchiveFile,
    ]);
    expect(filterObjects(sampleObjects, undefined, "glacier")).toEqual([glacierFile]);
  });

  it("combines text and type filters", () => {
    expect(filterObjects(sampleObjects, "old", "glacier")).toEqual([glacierFile]);
    expect(filterObjects(sampleObjects, "docs", "folders")).toEqual([folder]);
    expect(filterObjects(sampleObjects, "docs", "files")).toEqual([]);
  });

  it("returns empty array when nothing matches", () => {
    expect(filterObjects(sampleObjects, "missing")).toEqual([]);
    expect(filterObjects(sampleObjects, undefined, "glacier")).not.toContain(folder);
  });
});

describe("looksLikePath", () => {
  it("returns false for empty or whitespace-only values", () => {
    expect(looksLikePath("")).toBe(false);
    expect(looksLikePath("   ")).toBe(false);
    expect(looksLikePath("\t\n")).toBe(false);
  });

  it("returns true when value contains a slash", () => {
    expect(looksLikePath("photos/")).toBe(true);
    expect(looksLikePath("photos/cat.jpg")).toBe(true);
    expect(looksLikePath("  photos/  ")).toBe(true);
  });

  it("returns true for s3:// URIs case-insensitively", () => {
    expect(looksLikePath("s3://bucket/key")).toBe(true);
    expect(looksLikePath("S3://bucket/key")).toBe(true);
    expect(looksLikePath("  s3:bucket/key  ")).toBe(true);
  });

  it("returns false for plain filter text", () => {
    expect(looksLikePath("readme")).toBe(false);
    expect(looksLikePath("cat.jpg")).toBe(false);
    expect(looksLikePath("my-bucket")).toBe(false);
  });
});

describe("resolveDeleteTargets", () => {
  it("uses selected objects when the first argument is not an array", () => {
    const selected = [file];
    const event = { type: "click" } as unknown as S3Object[];

    expect(resolveDeleteTargets(event, selected)).toEqual(selected);
    expect(resolveDeleteTargets(undefined, selected)).toEqual(selected);
  });

  it("uses explicit object targets when an array is provided", () => {
    expect(resolveDeleteTargets([folder], [file])).toEqual([folder]);
  });
});
