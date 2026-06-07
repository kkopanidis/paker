import { invoke } from "@tauri-apps/api/core";

/** Structured error payload returned by Rust `PakerError` over IPC. */
export interface PakerIpcError {
  code: string;
  message: string;
  userAction?: string;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

/** Parse a rejected Tauri invoke error into a structured Paker IPC error. */
export function parseStructuredError(error: unknown): PakerIpcError {
  if (isRecord(error)) {
    const code = typeof error.code === "string" ? error.code : undefined;
    const message = typeof error.message === "string" ? error.message : undefined;
    if (code && message) {
      return {
        code,
        message,
        userAction: typeof error.userAction === "string" ? error.userAction : undefined,
      };
    }
  }

  if (typeof error === "string") {
    try {
      const parsed: unknown = JSON.parse(error);
      if (parsed !== error) {
        return parseStructuredError(parsed);
      }
    } catch {
      // plain string fallback
    }
    return { code: "unknown", message: error };
  }

  if (error instanceof Error) {
    try {
      const parsed: unknown = JSON.parse(error.message);
      if (parsed !== error.message) {
        return parseStructuredError(parsed);
      }
    } catch {
      // plain message fallback
    }
    return { code: "unknown", message: error.message };
  }

  return { code: "unknown", message: String(error) };
}

/** Invoke a Tauri command and rethrow structured `PakerIpcError` on failure. */
export async function invokeSafe<T>(
  cmd: string,
  args?: Record<string, unknown>
): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (error) {
    throw parseStructuredError(error);
  }
}
import type { S3Connection, S3ConnectionInput } from "@/types/connection";
import type {
  BucketIndexMeta,
  BucketInfo,
  BucketMetadata,
  CachedListResult,
  CopyMoveItem,
  IndexedObject,
  ListObjectsResponse,
  ObjectHeadResponse,
  ListObjectsResult,
  PrefixSizeResult,
  S3Object,
} from "@/types/s3";
import type { LocalEntry, TransferSettings } from "@/types/local";
import type {
  ConnectionNav,
  FullUiState,
  PanelLayout,
  PanelLayoutMode,
  PrefixBookmark,
  UiPreferences,
  UpdateInfo,
} from "@/types/ui";

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
  return invokeSafe<S3Connection>("save_connection", { input: mapConnectionInput(payload) });
}

export function deleteConnection(id: string): Promise<void> {
  return invoke<boolean>("delete_connection", { id }).then(() => undefined);
}

export function testConnection(id: string): Promise<void> {
  return invokeSafe<void>("test_connection", { id });
}

export function listBuckets(connectionId: string, forceAll = false): Promise<BucketInfo[]> {
  return invoke<BucketInfo[]>("list_buckets", { connectionId, forceAll });
}

export function verifyBucket(connectionId: string, bucket: string): Promise<void> {
  return invoke<void>("verify_bucket", { connectionId, bucket: bucket.trim() });
}

export function readListCache(
  connectionId: string,
  bucket: string,
  prefix?: string
): Promise<CachedListResult | null> {
  return invoke<CachedListResult | null>("read_list_cache", {
    connectionId,
    bucket,
    prefix: prefix ?? null,
  });
}

export function listObjects(
  connectionId: string,
  bucket: string,
  prefix?: string,
  continuationToken?: string,
  forceRefresh?: boolean
): Promise<ListObjectsResponse> {
  return invoke<ListObjectsResponse>("list_objects", {
    connectionId,
    bucket,
    prefix: prefix ?? null,
    continuationToken: continuationToken ?? null,
    forceRefresh: forceRefresh ?? false,
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
  key: string,
  forceRefresh?: boolean
): Promise<ObjectHeadResponse> {
  return invoke<ObjectHeadResponse>("head_object", {
    connectionId,
    bucket,
    key,
    forceRefresh: forceRefresh ?? false,
  });
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

export function calculatePrefixSize(
  connectionId: string,
  bucket: string,
  prefix?: string,
  forceRefresh?: boolean
): Promise<PrefixSizeResult> {
  return invoke<PrefixSizeResult>("calculate_prefix_size", {
    connectionId,
    bucket,
    prefix: prefix ?? null,
    forceRefresh: forceRefresh ?? false,
  });
}

export function getBucketMetadata(
  connectionId: string,
  bucket: string
): Promise<BucketMetadata> {
  return invoke<BucketMetadata>("get_bucket_metadata", { connectionId, bucket });
}

export function getFullUiState(): Promise<FullUiState> {
  return invoke<FullUiState>("get_full_ui_state");
}

export function getConnectionNav(connectionId: string): Promise<ConnectionNav | null> {
  return invoke<ConnectionNav | null>("get_connection_nav", { connectionId });
}

export function setConnectionNav(connectionId: string, nav: ConnectionNav): Promise<void> {
  return invoke<void>("set_connection_nav", { connectionId, nav });
}

export function getBookmarks(connectionId: string): Promise<PrefixBookmark[]> {
  return invoke<PrefixBookmark[]>("get_bookmarks", { connectionId });
}

export function addBookmark(connectionId: string, bookmark: PrefixBookmark): Promise<void> {
  return invoke<void>("add_bookmark", { connectionId, bookmark });
}

export function removeBookmark(connectionId: string, bookmarkId: string): Promise<void> {
  return invoke<void>("remove_bookmark", { connectionId, bookmarkId });
}

export function getUiPreferences(): Promise<UiPreferences> {
  return invoke<UiPreferences>("get_ui_preferences");
}

export function setUiPreferences(preferences: UiPreferences): Promise<void> {
  return invoke<void>("set_ui_preferences", { preferences });
}

export function getPanelLayout(mode: PanelLayoutMode): Promise<PanelLayout | null> {
  return invoke<PanelLayout | null>("get_panel_layout", { mode });
}

export function setPanelLayout(mode: PanelLayoutMode, layout: PanelLayout): Promise<void> {
  return invoke<void>("set_panel_layout", { mode, layout });
}

export function presignObject(
  connectionId: string,
  bucket: string,
  key: string,
  expiresSecs?: number
): Promise<string> {
  return invoke<string>("presign_object", {
    connectionId,
    bucket,
    key,
    expiresSecs: expiresSecs ?? null,
  });
}

export function previewObjectToCache(
  connectionId: string,
  bucket: string,
  key: string
): Promise<string> {
  return invoke<string>("preview_object_to_cache", { connectionId, bucket, key });
}

export function getBucketIndexStatus(
  connectionId: string,
  bucket: string
): Promise<BucketIndexMeta | null> {
  return invoke<BucketIndexMeta | null>("get_bucket_index_status", { connectionId, bucket });
}

export function startBucketIndex(
  connectionId: string,
  bucket: string,
  rebuild = true
): Promise<string> {
  return invoke<string>("start_bucket_index", { connectionId, bucket, rebuild });
}

export function pauseBucketIndex(connectionId: string, bucket: string): Promise<void> {
  return invoke<void>("pause_bucket_index", { connectionId, bucket });
}

export function resumeBucketIndex(connectionId: string, bucket: string): Promise<void> {
  return invoke<void>("resume_bucket_index", { connectionId, bucket });
}

export function cancelBucketIndex(connectionId: string, bucket: string): Promise<void> {
  return invoke<void>("cancel_bucket_index", { connectionId, bucket });
}

export function searchBucketIndex(
  connectionId: string,
  bucket: string,
  query: string,
  limit?: number,
  offset?: number
): Promise<IndexedObject[]> {
  return invoke<IndexedObject[]>("search_bucket_index", {
    connectionId,
    bucket,
    query,
    limit: limit ?? null,
    offset: offset ?? null,
  });
}

export function exportBucketIndexCsv(
  connectionId: string,
  bucket: string,
  savePath?: string
): Promise<string> {
  return invoke<string>("export_bucket_index_csv", {
    connectionId,
    bucket,
    savePath: savePath ?? null,
  });
}

export function checkForUpdate(): Promise<UpdateInfo> {
  return invoke<UpdateInfo>("check_for_update");
}
