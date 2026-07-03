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

/** Format a structured IPC error for toast display. */
export function formatIpcError(error: unknown): string {
  const parsed = parseStructuredError(error);
  if (parsed.userAction) {
    return `${parsed.message} ${parsed.userAction}`;
  }
  return parsed.message;
}
import type { S3Connection, S3ConnectionInput } from "@/types/connection";
import type {
  ChangeMasterKeyInput,
  SetupVaultInput,
  SetVaultPreferencesInput,
  VaultStatus,
} from "@/types/vault";
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
    skipTlsVerify: payload.skipTlsVerify,
    defaultBucket: payload.defaultBucket || null,
  };
}

export function listConnections(): Promise<S3Connection[]> {
  return invokeSafe<S3Connection[]>("list_connections");
}

export function getConnection(id: string): Promise<S3Connection | null> {
  return invokeSafe<S3Connection | null>("get_connection", { id });
}

export function saveConnection(payload: SaveConnectionPayload): Promise<S3Connection> {
  return invokeSafe<S3Connection>("save_connection", { input: mapConnectionInput(payload) });
}

export function deleteConnection(id: string): Promise<void> {
  return invokeSafe<boolean>("delete_connection", { id }).then(() => undefined);
}

export function testConnection(id: string): Promise<void> {
  return invokeSafe<void>("test_connection", { id });
}

export function listBuckets(connectionId: string, forceAll = false): Promise<BucketInfo[]> {
  return invokeSafe<BucketInfo[]>("list_buckets", { connectionId, forceAll });
}

export function verifyBucket(connectionId: string, bucket: string): Promise<void> {
  return invokeSafe<void>("verify_bucket", { connectionId, bucket: bucket.trim() });
}

export function readListCache(
  connectionId: string,
  bucket: string,
  prefix?: string
): Promise<CachedListResult | null> {
  return invokeSafe<CachedListResult | null>("read_list_cache", {
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
  return invokeSafe<ListObjectsResponse>("list_objects", {
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
    lastModified:
      result.objects.find((obj) => obj.key === key)?.lastModified ??
      result.prefixLastModified?.[key],
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
  return invokeSafe<ObjectHeadResponse>("head_object", {
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
  return invokeSafe<string[]>("check_objects_exist", { connectionId, bucket, keys });
}

export function pickUploadFiles(): Promise<string[]> {
  return invokeSafe<string[]>("pick_upload_files");
}

export function uploadFiles(
  connectionId: string,
  bucket: string,
  prefix: string,
  localPaths?: string[]
): Promise<string[]> {
  return invokeSafe<string[]>("upload_files", {
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
  return invokeSafe<string[]>("download_files", {
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
): Promise<{ deletedCount: number }> {
  return invokeSafe<{ deletedCount: number }>("delete_objects", {
    connectionId,
    bucket,
    keys,
  });
}

export function renameObject(
  connectionId: string,
  bucket: string,
  oldKey: string,
  newKey: string
): Promise<void> {
  return invokeSafe<void>("rename_object", { connectionId, bucket, oldKey, newKey });
}

export function createFolder(
  connectionId: string,
  bucket: string,
  prefix: string,
  folderName: string
): Promise<string> {
  return invokeSafe<string>("create_folder", {
    connectionId,
    bucket,
    prefix,
    folderName,
  });
}

export function cancelTransfer(transferId: string): Promise<void> {
  return invokeSafe<void>("cancel_transfer", { transferId });
}

export function pauseTransfer(transferId: string): Promise<void> {
  return invokeSafe<void>("pause_transfer", { transferId });
}

export function resumeTransfer(transferId: string): Promise<void> {
  return invokeSafe<void>("resume_transfer", { transferId });
}

export function copyObjects(
  connectionId: string,
  srcBucket: string,
  destBucket: string,
  items: CopyMoveItem[],
  destPrefix?: string
): Promise<void> {
  return invokeSafe<void>("copy_objects", {
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
  return invokeSafe<void>("move_objects", {
    connectionId,
    srcBucket,
    destBucket,
    items,
    destPrefix: destPrefix ?? null,
  });
}

export function listLocalDir(path: string): Promise<LocalEntry[]> {
  return invokeSafe<LocalEntry[]>("list_local_dir", { path });
}

export function getHomeDir(): Promise<string> {
  return invokeSafe<string>("get_home_dir");
}

export function pickLocalFolder(): Promise<string | null> {
  return invokeSafe<string | null>("pick_local_folder");
}

export function getParentPath(path: string): Promise<string | null> {
  return invokeSafe<string | null>("get_parent_path", { path });
}

export function getLastLocalDir(connectionId: string): Promise<string | null> {
  return invokeSafe<string | null>("get_last_local_dir", { connectionId });
}

export function setLastLocalDir(connectionId: string, path: string): Promise<void> {
  return invokeSafe<void>("set_last_local_dir", { connectionId, path });
}

export function getTransferSettings(): Promise<TransferSettings> {
  return invokeSafe<TransferSettings>("get_transfer_settings");
}

export function calculatePrefixSize(
  connectionId: string,
  bucket: string,
  prefix?: string,
  forceRefresh?: boolean
): Promise<PrefixSizeResult> {
  return invokeSafe<PrefixSizeResult>("calculate_prefix_size", {
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
  return invokeSafe<BucketMetadata>("get_bucket_metadata", { connectionId, bucket });
}

export function getFullUiState(): Promise<FullUiState> {
  return invokeSafe<FullUiState>("get_full_ui_state");
}

export function getConnectionNav(connectionId: string): Promise<ConnectionNav | null> {
  return invokeSafe<ConnectionNav | null>("get_connection_nav", { connectionId });
}

export function setConnectionNav(connectionId: string, nav: ConnectionNav): Promise<void> {
  return invokeSafe<void>("set_connection_nav", { connectionId, nav });
}

export function getBookmarks(connectionId: string): Promise<PrefixBookmark[]> {
  return invokeSafe<PrefixBookmark[]>("get_bookmarks", { connectionId });
}

export function addBookmark(connectionId: string, bookmark: PrefixBookmark): Promise<void> {
  return invokeSafe<void>("add_bookmark", { connectionId, bookmark });
}

export function removeBookmark(connectionId: string, bookmarkId: string): Promise<void> {
  return invokeSafe<void>("remove_bookmark", { connectionId, bookmarkId });
}

export function getUiPreferences(): Promise<UiPreferences> {
  return invokeSafe<UiPreferences>("get_ui_preferences");
}

export function setUiPreferences(preferences: UiPreferences): Promise<void> {
  return invokeSafe<void>("set_ui_preferences", { preferences });
}

export function getPanelLayout(mode: PanelLayoutMode): Promise<PanelLayout | null> {
  return invokeSafe<PanelLayout | null>("get_panel_layout", { mode });
}

export function setPanelLayout(mode: PanelLayoutMode, layout: PanelLayout): Promise<void> {
  return invokeSafe<void>("set_panel_layout", { mode, layout });
}

export function presignObject(
  connectionId: string,
  bucket: string,
  key: string,
  expiresSecs?: number
): Promise<string> {
  return invokeSafe<string>("presign_object", {
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
  return invokeSafe<string>("preview_object_to_cache", { connectionId, bucket, key });
}

export function getBucketIndexStatus(
  connectionId: string,
  bucket: string
): Promise<BucketIndexMeta | null> {
  return invokeSafe<BucketIndexMeta | null>("get_bucket_index_status", { connectionId, bucket });
}

export function startBucketIndex(
  connectionId: string,
  bucket: string,
  rebuild = true
): Promise<string> {
  return invokeSafe<string>("start_bucket_index", { connectionId, bucket, rebuild });
}

export function pauseBucketIndex(connectionId: string, bucket: string): Promise<void> {
  return invokeSafe<void>("pause_bucket_index", { connectionId, bucket });
}

export function resumeBucketIndex(connectionId: string, bucket: string): Promise<void> {
  return invokeSafe<void>("resume_bucket_index", { connectionId, bucket });
}

export function cancelBucketIndex(connectionId: string, bucket: string): Promise<void> {
  return invokeSafe<void>("cancel_bucket_index", { connectionId, bucket });
}

export function searchBucketIndex(
  connectionId: string,
  bucket: string,
  query: string,
  limit?: number,
  offset?: number
): Promise<IndexedObject[]> {
  return invokeSafe<IndexedObject[]>("search_bucket_index", {
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
  return invokeSafe<string>("export_bucket_index_csv", {
    connectionId,
    bucket,
    savePath: savePath ?? null,
  });
}

export function checkForUpdate(): Promise<UpdateInfo> {
  return invokeSafe<UpdateInfo>("check_for_update");
}

export function openPreviewFile(path: string): Promise<void> {
  return invokeSafe<void>("open_preview_file", { path });
}

export function getVaultStatus(): Promise<VaultStatus> {
  return invokeSafe<VaultStatus>("get_vault_status");
}

export function setupVault(input: SetupVaultInput): Promise<void> {
  return invokeSafe<void>("setup_vault", {
    masterPassword: input.masterPassword,
    autoLockMinutes: input.autoLockMinutes ?? 15,
    lockOnBlur: input.lockOnBlur ?? false,
  });
}

export function unlockVault(masterPassword: string): Promise<void> {
  return invokeSafe<void>("unlock_vault", { masterPassword });
}

export function lockVault(): Promise<void> {
  return invokeSafe<void>("lock_vault");
}

export function changeMasterKey(input: ChangeMasterKeyInput): Promise<void> {
  return invokeSafe<void>("change_master_key", { ...input });
}

export function resetMasterKeyWithOsAuth(newPassword: string): Promise<void> {
  return invokeSafe<void>("reset_master_key_with_os_auth", { newPassword });
}

export function setVaultPreferences(input: SetVaultPreferencesInput): Promise<void> {
  return invokeSafe<void>("set_vault_preferences", { ...input });
}

export function recordVaultActivity(): Promise<void> {
  return invokeSafe<void>("record_vault_activity");
}

export function dismissVaultPrompt(): Promise<void> {
  return invokeSafe<void>("dismiss_vault_prompt");
}

export function getVaultPromptDismissed(): Promise<boolean> {
  return invokeSafe<boolean>("get_vault_prompt_dismissed");
}
