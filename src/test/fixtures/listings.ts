import type { BucketInfo, ListObjectsResponse, S3Object } from "@/types/s3";

export const emptyListing: ListObjectsResponse = {
  objects: [],
  commonPrefixes: [],
  continuationToken: undefined,
  isTruncated: false,
  fromCache: false,
  fetchedAt: "1700000000",
};

export const sampleBuckets: BucketInfo[] = [
  { name: "bucket-a", creationDate: "2024-01-01T00:00:00Z" },
  { name: "bucket-b", creationDate: "2024-02-01T00:00:00Z" },
];

export const sampleObjects: S3Object[] = [
  {
    key: "docs/",
    name: "docs",
    isFolder: true,
    size: 0,
  },
  {
    key: "docs/readme.txt",
    name: "readme.txt",
    isFolder: false,
    size: 1024,
    storageClass: "STANDARD",
    lastModified: "2024-06-01T12:00:00Z",
  },
  {
    key: "archive/old.zip",
    name: "old.zip",
    isFolder: false,
    size: 4096,
    storageClass: "GLACIER",
    lastModified: "2024-05-01T12:00:00Z",
  },
  {
    key: "photos/cat.jpg",
    name: "cat.jpg",
    isFolder: false,
    size: 2048,
    storageClass: "STANDARD",
    lastModified: "2024-04-01T12:00:00Z",
  },
];
