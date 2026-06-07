export interface BucketInfo {
  name: string;
  creationDate?: string;
}

export interface ObjectInfo {
  key: string;
  size: number;
  lastModified?: string;
  etag?: string;
  storageClass?: string;
  isPrefix?: boolean;
}

export interface S3Object {
  key: string;
  name: string;
  isFolder: boolean;
  size: number;
  lastModified?: string;
  storageClass?: string;
  etag?: string;
}

export interface ObjectHeadDetails {
  key: string;
  contentType?: string;
  contentLength?: number;
  lastModified?: string;
  etag?: string;
  storageClass?: string;
  metadata?: Record<string, string>;
}

export interface ListObjectsResult {
  objects: ObjectInfo[];
  commonPrefixes?: string[];
  continuationToken?: string;
  isTruncated: boolean;
}

export interface CachedListResult {
  result: ListObjectsResult;
  fetchedAt: string;
}

export interface ListObjectsResponse extends ListObjectsResult {
  fromCache?: boolean;
  fetchedAt?: string;
}

export interface ObjectHeadResponse extends ObjectHeadDetails {
  fromCache?: boolean;
  fetchedAt?: string;
}

export interface TransferProgress {
  transferId: string;
  fileName: string;
  direction: "upload" | "download" | "copy";
  bytes: number;
  total: number;
  status: "started" | "in_progress" | "completed" | "failed" | "cancelled" | "paused";
}

export interface CopyMoveItem {
  srcKey: string;
  destKey?: string;
}

export interface PrefixSizeResult {
  prefix: string;
  objectCount: number;
  totalBytes: number;
}

export interface PrefixSizeProgress {
  prefix: string;
  objectCount: number;
  totalBytes: number;
  done: boolean;
  error?: string;
}

export interface BucketIndexMeta {
  connectionId: string;
  bucket: string;
  status: "idle" | "running" | "paused" | "completed" | "stale" | "failed" | "cancelled";
  objectCount: number;
  startedAt?: string;
  completedAt?: string;
  error?: string;
}

export interface BucketIndexProgress {
  connectionId: string;
  bucket: string;
  objectCount: number;
  status: BucketIndexMeta["status"];
  done: boolean;
  error?: string;
}

export interface IndexedObject {
  key: string;
  size: number;
  lastModified?: string;
  etag?: string;
  storageClass?: string;
}

export interface BucketMetadata {
  name: string;
  creationDate?: string;
  location?: string;
  versioning?: string;
  connectionName?: string;
  endpoint?: string;
  region?: string;
  forcePathStyle?: boolean;
}
