import { invoke } from "@tauri-apps/api/core";
import type { S3Connection, S3ConnectionInput } from "@/types/connection";
import type { BucketInfo, CopyMoveItem, ListObjectsResult, ObjectHeadDetails, S3Object } from "@/types/s3";
import type { LocalEntry, TransferSettings } from "@/types/local";

export interface SaveConnectionPayload extends S3ConnectionInput {
  id?: string;
}

function mapConnectionInput(payload: SaveConnectionPayload) {
  return {
    id: payload.id,
    name: payload.name,
    endpoint: payload.endpoint || null,
    region: payload.region,
    accessKeyId: payload.accessKeyId,
    secretAccessKey: payload.secretAccessKey || null,
    sessionToken: payload.sessionToken || null,
    forcePathStyle: payload.forcePathStyle,
    defaultBucket: payload.defaultBucket || null,
  };
}

export function listConnections(): Promise<S3Connection[]> {
  return invoke<S3Connection[]>("list_connections");
}

export function getConnection(id: string): Promise<S3Connection | null> {
  return invoke<S3Connection | null>("get_connection", { id });
}

export function saveConnection(payload: SaveConnectionPayload): Promise<S3Connection> {
  return invoke<S3Connection>("save_connection", { input: mapConnectionInput(payload) });
}

export function deleteConnection(id: string): Promise<void> {
  return invoke<boolean>("delete_connection", { id }).then(() => undefined);
}

export function testConnection(id: string): Promise<void> {
  return invoke<void>("test_connection", { id });
}

export function listBuckets(connectionId: string, forceAll = false): Promise<BucketInfo[]> {
  return invoke<BucketInfo[]>("list_buckets", { connectionId, forceAll });
}

export function verifyBucket(connectionId: string, bucket: string): Promise<void> {
  return invoke<void>("verify_bucket", { connectionId, bucket: bucket.trim() });
}

export function listObjects(
  connectionId: string,
  bucket: string,
  prefix?: string,
  continuationToken?: string
): Promise<ListObjectsResult> {
  return invoke<ListObjectsResult>("list_objects", {
    connectionId,
    bucket,
    prefix: prefix ?? null,
    continuationToken: continuationToken ?? null,
  });
}

export function normalizeObjects(result: ListObjectsResult, prefix: string): S3Object[] {
  const folderKeys = new Set(result.commonPrefixes ?? []);

  const folders: S3Object[] = [...folderKeys].map((key) => ({
    key,
    name: key.slice(prefix.length).replace(/\/$/, ""),
    isFolder: true,
    size: 0,
  }));

  const files: S3Object[] = result.objects
    .filter((obj) => !obj.key.endsWith("/") || !folderKeys.has(obj.key))
    .map((obj) => ({
      key: obj.key,
      name: obj.key.slice(prefix.length).replace(/\/$/, ""),
      isFolder: obj.isPrefix ?? false,
      size: Number(obj.size ?? 0),
      lastModified: obj.lastModified,
      storageClass: obj.storageClass,
      etag: obj.etag,
    }));

  return [...folders, ...files].sort((a, b) => {
    if (a.isFolder !== b.isFolder) return a.isFolder ? -1 : 1;
    return a.name.localeCompare(b.name);
  });
}

export function headObject(
  connectionId: string,
  bucket: string,
  key: string
): Promise<ObjectHeadDetails> {
  return invoke<ObjectHeadDetails>("head_object", { connectionId, bucket, key });
}

export function checkObjectsExist(
  connectionId: string,
  bucket: string,
  keys: string[]
): Promise<string[]> {
  return invoke<string[]>("check_objects_exist", { connectionId, bucket, keys });
}

export function pickUploadFiles(): Promise<string[]> {
  return invoke<string[]>("pick_upload_files");
}

export function uploadFiles(
  connectionId: string,
  bucket: string,
  prefix: string,
  localPaths?: string[]
): Promise<string[]> {
  return invoke<string[]>("upload_files", {
    connectionId,
    bucket,
    prefix,
    localPaths: localPaths ?? [],
  });
}

export function downloadFiles(
  connectionId: string,
  bucket: string,
  keys: string[],
  saveDir?: string
): Promise<string[]> {
  return invoke<string[]>("download_files", {
    connectionId,
    bucket,
    keys,
    saveDir: saveDir ?? null,
  });
}

export function deleteObjects(
  connectionId: string,
  bucket: string,
  keys: string[]
): Promise<void> {
  return invoke<void>("delete_objects", { connectionId, bucket, keys });
}

export function renameObject(
  connectionId: string,
  bucket: string,
  oldKey: string,
  newKey: string
): Promise<void> {
  return invoke<void>("rename_object", { connectionId, bucket, oldKey, newKey });
}

export function createFolder(
  connectionId: string,
  bucket: string,
  prefix: string,
  folderName: string
): Promise<string> {
  return invoke<string>("create_folder", {
    connectionId,
    bucket,
    prefix,
    folderName,
  });
}

export function cancelTransfer(transferId: string): Promise<void> {
  return invoke<void>("cancel_transfer", { transferId });
}

export function pauseTransfer(transferId: string): Promise<void> {
  return invoke<void>("pause_transfer", { transferId });
}

export function resumeTransfer(transferId: string): Promise<void> {
  return invoke<void>("resume_transfer", { transferId });
}

export function copyObjects(
  connectionId: string,
  srcBucket: string,
  destBucket: string,
  items: CopyMoveItem[],
  destPrefix?: string
): Promise<void> {
  return invoke<void>("copy_objects", {
    connectionId,
    srcBucket,
    destBucket,
    items,
    destPrefix: destPrefix ?? null,
  });
}

export function moveObjects(
  connectionId: string,
  srcBucket: string,
  destBucket: string,
  items: CopyMoveItem[],
  destPrefix?: string
): Promise<void> {
  return invoke<void>("move_objects", {
    connectionId,
    srcBucket,
    destBucket,
    items,
    destPrefix: destPrefix ?? null,
  });
}

export function listLocalDir(path: string): Promise<LocalEntry[]> {
  return invoke<LocalEntry[]>("list_local_dir", { path });
}

export function getHomeDir(): Promise<string> {
  return invoke<string>("get_home_dir");
}

export function pickLocalFolder(): Promise<string | null> {
  return invoke<string | null>("pick_local_folder");
}

export function getParentPath(path: string): Promise<string | null> {
  return invoke<string | null>("get_parent_path", { path });
}

export function getLastLocalDir(connectionId: string): Promise<string | null> {
  return invoke<string | null>("get_last_local_dir", { connectionId });
}

export function setLastLocalDir(connectionId: string, path: string): Promise<void> {
  return invoke<void>("set_last_local_dir", { connectionId, path });
}

export function getTransferSettings(): Promise<TransferSettings> {
  return invoke<TransferSettings>("get_transfer_settings");
}
