import type { S3Object } from "@/types/s3";

export type TypeFilter = "all" | "folders" | "files" | "glacier";

export function matchesTypeFilter(object: S3Object, typeFilter: TypeFilter): boolean {
  switch (typeFilter) {
    case "folders":
      return object.isFolder;
    case "files":
      return !object.isFolder;
    case "glacier":
      return object.storageClass?.toUpperCase().includes("GLACIER") ?? false;
    default:
      return true;
  }
}

export function filterObjects(
  objects: S3Object[],
  filterText?: string,
  typeFilter?: TypeFilter
): S3Object[] {
  let result = objects;
  const query = filterText?.trim().toLowerCase();
  if (query) {
    result = result.filter((object) => object.name.toLowerCase().includes(query));
  }
  if (typeFilter && typeFilter !== "all") {
    result = result.filter((object) => matchesTypeFilter(object, typeFilter));
  }
  return result;
}

export function looksLikePath(value: string): boolean {
  const trimmed = value.trim();
  if (!trimmed) return false;
  if (trimmed.includes("/")) return true;
  return /^s3:/i.test(trimmed);
}
