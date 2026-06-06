export interface BucketInfo {
  name: string;
  creationDate?: string;
}

export interface ObjectInfo {
  key: string;
  size: number;
  lastModified?: string;
  storageClass?: string;
  etag?: string;
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
