use crate::error::{into_ipc_error, map_s3_sdk_error, PakerError};
use anyhow::{anyhow, Context, Result};
use aws_sdk_s3::error::SdkError;
use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::types::{CompletedMultipartUpload, CompletedPart, ObjectIdentifier};
use aws_sdk_s3::Client;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, Manager};
use tokio::fs;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::path_safety::local_dest_path as safe_local_dest_path;
use crate::storage::bucket_index::{BucketIndexMeta, BucketIndexProgress, IndexedObject};
use crate::storage::ObjectCacheManager;
use crate::transfer::TransferManager;

const MULTIPART_THRESHOLD: u64 = 5 * 1024 * 1024;
const PART_SIZE: usize = 5 * 1024 * 1024;
const PREVIEW_MAX_BYTES: u64 = 5 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferProgress {
    pub transfer_id: String,
    pub file_name: String,
    pub direction: String,
    pub bytes: u64,
    pub total: u64,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BucketInfo {
    pub name: String,
    pub creation_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectInfo {
    pub key: String,
    pub size: i64,
    pub last_modified: Option<String>,
    pub etag: Option<String>,
    pub storage_class: Option<String>,
    pub is_prefix: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListObjectsResult {
    pub objects: Vec<ObjectInfo>,
    pub common_prefixes: Vec<String>,
    pub continuation_token: Option<String>,
    pub is_truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrefixSizeResult {
    pub prefix: String,
    pub object_count: u64,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrefixSizeProgress {
    pub prefix: String,
    pub object_count: u64,
    pub total_bytes: u64,
    pub done: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BucketMetadata {
    pub name: String,
    pub creation_date: Option<String>,
    pub location: Option<String>,
    pub versioning: Option<String>,
    pub connection_name: Option<String>,
    pub endpoint: Option<String>,
    pub region: Option<String>,
    pub force_path_style: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectHeadResult {
    pub key: String,
    pub content_type: Option<String>,
    pub content_length: Option<i64>,
    pub last_modified: Option<String>,
    pub etag: Option<String>,
    pub metadata: HashMap<String, String>,
    pub storage_class: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedListResponse {
    pub result: ListObjectsResult,
    pub fetched_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListObjectsResponse {
    #[serde(flatten)]
    pub result: ListObjectsResult,
    pub from_cache: bool,
    pub fetched_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectHeadResponse {
    #[serde(flatten)]
    pub result: ObjectHeadResult,
    pub from_cache: bool,
    pub fetched_at: Option<String>,
}

pub fn object_info_to_head(obj: &ObjectInfo) -> ObjectHeadResult {
    ObjectHeadResult {
        key: obj.key.clone(),
        content_type: None,
        content_length: Some(obj.size),
        last_modified: obj.last_modified.clone(),
        etag: obj.etag.clone(),
        metadata: HashMap::new(),
        storage_class: obj.storage_class.clone(),
    }
}

fn emit_progress(
    app: &AppHandle,
    transfer_id: &str,
    file_name: &str,
    direction: &str,
    bytes: u64,
    total: u64,
    status: &str,
) {
    let _ = app.emit(
        "transfer-progress",
        TransferProgress {
            transfer_id: transfer_id.to_string(),
            file_name: file_name.to_string(),
            direction: direction.to_string(),
            bytes,
            total,
            status: status.to_string(),
        },
    );
}

/// Check cancel token and, if the manager is available, block while the transfer is paused.
/// Returns true if the transfer was cancelled during a pause wait.
fn is_cancelled(cancel_token: &Option<CancellationToken>) -> bool {
    cancel_token.as_ref().is_some_and(|t| t.is_cancelled())
}

async fn wait_if_paused(
    app: &AppHandle,
    transfer_id: &str,
    file_name: &str,
    direction: &str,
    bytes: u64,
    total: u64,
    cancel_token: &Option<CancellationToken>,
) -> bool {
    let paused = app
        .try_state::<TransferManager>()
        .is_some_and(|m| m.is_paused(transfer_id));

    if !paused {
        return false;
    }

    emit_progress(app, transfer_id, file_name, direction, bytes, total, "paused");

    loop {
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        if is_cancelled(cancel_token) {
            return true;
        }

        let still_paused = app
            .try_state::<TransferManager>()
            .is_some_and(|m| m.is_paused(transfer_id));

        if !still_paused {
            break;
        }
    }

    false
}

pub async fn list_buckets(client: &Client) -> Result<Vec<BucketInfo>, PakerError> {
    let response = client
        .list_buckets()
        .send()
        .await
        .map_err(map_s3_sdk_error)?;

    Ok(response
        .buckets()
        .iter()
        .filter_map(|bucket| {
            bucket.name().map(|name| BucketInfo {
                name: name.to_string(),
                creation_date: bucket.creation_date().map(|dt| dt.to_string()),
            })
        })
        .collect())
}

pub async fn presign_get_object(
    client: &Client,
    bucket: &str,
    key: &str,
    expires_secs: u64,
) -> Result<String, PakerError> {
    let presigned = client
        .get_object()
        .bucket(bucket)
        .key(key)
        .presigned(
            PresigningConfig::expires_in(Duration::from_secs(expires_secs))
                .map_err(|_| PakerError::InvalidInput("Invalid presign expiry".to_string()))?,
        )
        .await
        .map_err(map_s3_sdk_error)?;

    Ok(presigned.uri().to_string())
}

pub async fn head_object(
    client: &Client,
    bucket: &str,
    key: &str,
) -> Result<ObjectHeadResult, PakerError> {
    let response = client
        .head_object()
        .bucket(bucket)
        .key(key)
        .send()
        .await
        .map_err(map_s3_sdk_error)?;

    let metadata = response
        .metadata()
        .map(|entries| {
            entries
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect()
        })
        .unwrap_or_default();

    Ok(ObjectHeadResult {
        key: key.to_string(),
        content_type: response.content_type().map(|s| s.to_string()),
        content_length: response.content_length(),
        last_modified: response.last_modified().map(|dt| dt.to_string()),
        etag: response.e_tag().map(|s| s.to_string()),
        metadata,
        storage_class: response
            .storage_class()
            .map(|class| class.as_str().to_string()),
    })
}

pub async fn object_exists(client: &Client, bucket: &str, key: &str) -> Result<bool, PakerError> {
    match client.head_object().bucket(bucket).key(key).send().await {
        Ok(_) => Ok(true),
        Err(SdkError::ServiceError(err)) if err.err().is_not_found() => Ok(false),
        Err(err) => Err(map_s3_sdk_error(err)),
    }
}

pub async fn verify_bucket_access(client: &Client, bucket: &str) -> Result<(), PakerError> {
    match client.head_bucket().bucket(bucket).send().await {
        Ok(_) => return Ok(()),
        Err(_) => {
            client
                .list_objects_v2()
                .bucket(bucket)
                .max_keys(1)
                .send()
                .await
                .map_err(map_s3_sdk_error)?;
        }
    }
    Ok(())
}

fn normalize_prefix(prefix: &str) -> String {
    if prefix.is_empty() {
        return String::new();
    }
    if prefix.ends_with('/') {
        prefix.to_string()
    } else {
        format!("{prefix}/")
    }
}

fn emit_prefix_size_progress(app: &AppHandle, progress: PrefixSizeProgress) {
    let _ = app.emit("prefix-size-progress", progress);
}

fn emit_bucket_index_progress(app: &AppHandle, progress: BucketIndexProgress) {
    let _ = app.emit("bucket-index-progress", progress);
}

fn timestamp_now() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}

async fn wait_if_index_paused(
    app: &AppHandle,
    job_id: &str,
    connection_id: &str,
    bucket: &str,
    object_count: u64,
    cancel_token: &CancellationToken,
) -> bool {
    let paused = app
        .try_state::<TransferManager>()
        .is_some_and(|m| m.is_paused(job_id));

    if !paused {
        return false;
    }

    emit_bucket_index_progress(
        app,
        BucketIndexProgress {
            connection_id: connection_id.to_string(),
            bucket: bucket.to_string(),
            object_count,
            status: "paused".to_string(),
            done: false,
            error: None,
        },
    );

    loop {
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        if cancel_token.is_cancelled() {
            return true;
        }

        let still_paused = app
            .try_state::<TransferManager>()
            .is_some_and(|m| m.is_paused(job_id));

        if !still_paused {
            break;
        }
    }

    false
}

#[allow(clippy::too_many_arguments)]
pub async fn index_bucket_flat(
    app: &AppHandle,
    client: &Client,
    cache: &ObjectCacheManager,
    connection_id: &str,
    bucket: &str,
    job_id: &str,
    cancel_token: CancellationToken,
    rebuild: bool,
) -> Result<()> {
    let started_at = timestamp_now();

    if rebuild {
        cache.clear_bucket_index(connection_id, bucket)?;
    }

    cache.upsert_bucket_index_meta(&BucketIndexMeta {
        connection_id: connection_id.to_string(),
        bucket: bucket.to_string(),
        status: "running".to_string(),
        object_count: 0,
        started_at: Some(started_at.clone()),
        completed_at: None,
        error: None,
    })?;

    let mut continuation_token: Option<String> = None;
    let mut object_count: u64 = 0;

    emit_bucket_index_progress(
        app,
        BucketIndexProgress {
            connection_id: connection_id.to_string(),
            bucket: bucket.to_string(),
            object_count: 0,
            status: "running".to_string(),
            done: false,
            error: None,
        },
    );

    loop {
        if cancel_token.is_cancelled() {
            cache.upsert_bucket_index_meta(&BucketIndexMeta {
                connection_id: connection_id.to_string(),
                bucket: bucket.to_string(),
                status: "cancelled".to_string(),
                object_count,
                started_at: Some(started_at.clone()),
                completed_at: Some(timestamp_now()),
                error: None,
            })?;
            emit_bucket_index_progress(
                app,
                BucketIndexProgress {
                    connection_id: connection_id.to_string(),
                    bucket: bucket.to_string(),
                    object_count,
                    status: "cancelled".to_string(),
                    done: true,
                    error: None,
                },
            );
            return Ok(());
        }

        if wait_if_index_paused(
            app,
            job_id,
            connection_id,
            bucket,
            object_count,
            &cancel_token,
        )
        .await
        {
            cache.upsert_bucket_index_meta(&BucketIndexMeta {
                connection_id: connection_id.to_string(),
                bucket: bucket.to_string(),
                status: "cancelled".to_string(),
                object_count,
                started_at: Some(started_at.clone()),
                completed_at: Some(timestamp_now()),
                error: None,
            })?;
            emit_bucket_index_progress(
                app,
                BucketIndexProgress {
                    connection_id: connection_id.to_string(),
                    bucket: bucket.to_string(),
                    object_count,
                    status: "cancelled".to_string(),
                    done: true,
                    error: None,
                },
            );
            return Ok(());
        }

        let mut request = client
            .list_objects_v2()
            .bucket(bucket)
            .max_keys(1000);

        if let Some(token) = continuation_token.as_deref() {
            request = request.continuation_token(token);
        }

        let response = match request.send().await {
            Ok(response) => response,
            Err(err) => {
                let message = err.to_string();
                cache.upsert_bucket_index_meta(&BucketIndexMeta {
                    connection_id: connection_id.to_string(),
                    bucket: bucket.to_string(),
                    status: "failed".to_string(),
                    object_count,
                    started_at: Some(started_at.clone()),
                    completed_at: Some(timestamp_now()),
                    error: Some(message.clone()),
                })?;
                emit_bucket_index_progress(
                    app,
                    BucketIndexProgress {
                        connection_id: connection_id.to_string(),
                        bucket: bucket.to_string(),
                        object_count,
                        status: "failed".to_string(),
                        done: true,
                        error: Some(message.clone()),
                    },
                );
                return Err(anyhow!("index_bucket_flat failed: {message}"));
            }
        };

        let batch: Vec<IndexedObject> = response
            .contents()
            .iter()
            .filter_map(|object| {
                object.key().map(|key| IndexedObject {
                    key: key.to_string(),
                    size: object.size().unwrap_or_default(),
                    last_modified: object.last_modified().map(|dt| dt.to_string()),
                    etag: object.e_tag().map(|etag| etag.to_string()),
                    storage_class: object
                        .storage_class()
                        .map(|class| class.as_str().to_string()),
                })
            })
            .collect();

        object_count += batch.len() as u64;
        cache.upsert_indexed_objects_batch(connection_id, bucket, &batch)?;

        cache.upsert_bucket_index_meta(&BucketIndexMeta {
            connection_id: connection_id.to_string(),
            bucket: bucket.to_string(),
            status: "running".to_string(),
            object_count,
            started_at: Some(started_at.clone()),
            completed_at: None,
            error: None,
        })?;

        emit_bucket_index_progress(
            app,
            BucketIndexProgress {
                connection_id: connection_id.to_string(),
                bucket: bucket.to_string(),
                object_count,
                status: "running".to_string(),
                done: false,
                error: None,
            },
        );

        if !response.is_truncated().unwrap_or(false) {
            break;
        }

        continuation_token = response.next_continuation_token().map(|s| s.to_string());
        if continuation_token.is_none() {
            break;
        }
    }

    cache.upsert_bucket_index_meta(&BucketIndexMeta {
        connection_id: connection_id.to_string(),
        bucket: bucket.to_string(),
        status: "completed".to_string(),
        object_count,
        started_at: Some(started_at),
        completed_at: Some(timestamp_now()),
        error: None,
    })?;

    emit_bucket_index_progress(
        app,
        BucketIndexProgress {
            connection_id: connection_id.to_string(),
            bucket: bucket.to_string(),
            object_count,
            status: "completed".to_string(),
            done: true,
            error: None,
        },
    );

    Ok(())
}

pub async fn calculate_prefix_size(
    app: &AppHandle,
    client: &Client,
    bucket: &str,
    prefix: &str,
) -> Result<PrefixSizeResult, PakerError> {
    let normalized = if prefix.is_empty() {
        String::new()
    } else {
        normalize_prefix(prefix)
    };

    let mut continuation_token: Option<String> = None;
    let mut object_count: u64 = 0;
    let mut total_bytes: u64 = 0;

    loop {
        let mut request = client
            .list_objects_v2()
            .bucket(bucket)
            .max_keys(1000);

        if !normalized.is_empty() {
            request = request.prefix(&normalized);
        }

        if let Some(token) = continuation_token.as_deref() {
            request = request.continuation_token(token);
        }

        let response = request.send().await.map_err(map_s3_sdk_error)?;

        for object in response.contents() {
            let Some(key) = object.key() else {
                continue;
            };
            let size = object.size().unwrap_or_default().max(0) as u64;
            if key.ends_with('/') && size == 0 {
                continue;
            }
            object_count += 1;
            total_bytes += size;
        }

        emit_prefix_size_progress(
            app,
            PrefixSizeProgress {
                prefix: normalized.clone(),
                object_count,
                total_bytes,
                done: false,
                error: None,
            },
        );

        if !response.is_truncated().unwrap_or(false) {
            break;
        }

        continuation_token = response
            .next_continuation_token()
            .map(|s| s.to_string());

        if continuation_token.is_none() {
            break;
        }
    }

    let result = PrefixSizeResult {
        prefix: normalized,
        object_count,
        total_bytes,
    };

    emit_prefix_size_progress(
        app,
        PrefixSizeProgress {
            prefix: result.prefix.clone(),
            object_count: result.object_count,
            total_bytes: result.total_bytes,
            done: true,
            error: None,
        },
    );

    Ok(result)
}

pub async fn get_bucket_metadata(
    client: &Client,
    bucket: &str,
    creation_date: Option<String>,
    connection_name: Option<String>,
    endpoint: Option<String>,
    region: Option<String>,
    force_path_style: Option<bool>,
) -> Result<BucketMetadata, PakerError> {
    let location = match client.get_bucket_location().bucket(bucket).send().await {
        Ok(response) => {
            let constraint = response
                .location_constraint()
                .map(|c| c.as_str().to_string())
                .unwrap_or_default();
            Some(if constraint.is_empty() {
                "us-east-1".to_string()
            } else {
                constraint
            })
        }
        Err(_) => None,
    };

    let versioning = match client.get_bucket_versioning().bucket(bucket).send().await {
        Ok(response) => response
            .status()
            .map(|status| status.as_str().to_string()),
        Err(_) => None,
    };

    Ok(BucketMetadata {
        name: bucket.to_string(),
        creation_date,
        location,
        versioning,
        connection_name,
        endpoint,
        region,
        force_path_style,
    })
}

pub async fn list_objects_v2(
    client: &Client,
    bucket: &str,
    prefix: Option<&str>,
    continuation_token: Option<&str>,
) -> Result<ListObjectsResult, PakerError> {
    let mut request = client.list_objects_v2().bucket(bucket).delimiter("/");

    if let Some(prefix) = prefix {
        if !prefix.is_empty() {
            request = request.prefix(prefix);
        }
    }

    if let Some(token) = continuation_token {
        if !token.is_empty() {
            request = request.continuation_token(token);
        }
    }

    let response = request.send().await.map_err(map_s3_sdk_error)?;

    let objects = response
        .contents()
        .iter()
        .filter_map(|object| {
            object.key().map(|key| ObjectInfo {
                key: key.to_string(),
                size: object.size().unwrap_or_default(),
                last_modified: object.last_modified().map(|dt| dt.to_string()),
                etag: object.e_tag().map(|etag| etag.to_string()),
                storage_class: object
                    .storage_class()
                    .map(|class| class.as_str().to_string()),
                is_prefix: false,
            })
        })
        .collect();

    let common_prefixes = response
        .common_prefixes()
        .iter()
        .filter_map(|prefix| prefix.prefix().map(|p| p.to_string()))
        .collect();

    Ok(ListObjectsResult {
        objects,
        common_prefixes,
        continuation_token: response.next_continuation_token().map(|s| s.to_string()),
        is_truncated: response.is_truncated().unwrap_or(false),
    })
}

async fn put_object_single(
    client: &Client,
    bucket: &str,
    key: &str,
    body: Vec<u8>,
) -> Result<()> {
    client
        .put_object()
        .bucket(bucket)
        .key(key)
        .body(ByteStream::from(body))
        .send()
        .await
        .context("put_object failed")?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn put_object_multipart(
    app: &AppHandle,
    client: &Client,
    bucket: &str,
    key: &str,
    path: &Path,
    transfer_id: &str,
    file_name: &str,
    total: u64,
    cancel_token: &Option<CancellationToken>,
) -> Result<()> {
    let create = client
        .create_multipart_upload()
        .bucket(bucket)
        .key(key)
        .send()
        .await
        .context("create_multipart_upload failed")?;

    let upload_id = create
        .upload_id()
        .ok_or_else(|| anyhow!("multipart upload missing upload_id"))?
        .to_string();

    let mut file = fs::File::open(path)
        .await
        .with_context(|| format!("failed to open {}", path.display()))?;
    let mut part_number = 1i32;
    let mut uploaded: u64 = 0;
    let mut completed_parts = Vec::new();

    loop {
        // Cancel check
        if is_cancelled(cancel_token) {
            let _ = client
                .abort_multipart_upload()
                .bucket(bucket)
                .key(key)
                .upload_id(&upload_id)
                .send()
                .await;
            emit_progress(
                app,
                transfer_id,
                file_name,
                "upload",
                uploaded,
                total,
                "cancelled",
            );
            return Err(anyhow!("transfer cancelled"));
        }

        // Pause check
        let cancelled_during_pause = wait_if_paused(
            app,
            transfer_id,
            file_name,
            "upload",
            uploaded,
            total,
            cancel_token,
        )
        .await;
        if cancelled_during_pause {
            let _ = client
                .abort_multipart_upload()
                .bucket(bucket)
                .key(key)
                .upload_id(&upload_id)
                .send()
                .await;
            emit_progress(
                app,
                transfer_id,
                file_name,
                "upload",
                uploaded,
                total,
                "cancelled",
            );
            return Err(anyhow!("transfer cancelled"));
        }

        let mut buffer = vec![0u8; PART_SIZE];
        let read = file
            .read(&mut buffer)
            .await
            .with_context(|| format!("failed to read {}", path.display()))?;
        if read == 0 {
            break;
        }
        buffer.truncate(read);

        let upload_part = client
            .upload_part()
            .bucket(bucket)
            .key(key)
            .upload_id(&upload_id)
            .part_number(part_number)
            .body(ByteStream::from(buffer))
            .send()
            .await
            .with_context(|| format!("upload_part {part_number} failed"))?;

        let etag = upload_part
            .e_tag()
            .ok_or_else(|| anyhow!("upload_part {part_number} missing etag"))?
            .to_string();

        completed_parts.push(
            CompletedPart::builder()
                .part_number(part_number)
                .e_tag(etag)
                .build(),
        );

        uploaded += read as u64;
        emit_progress(
            app,
            transfer_id,
            file_name,
            "upload",
            uploaded,
            total,
            "in_progress",
        );
        part_number += 1;
    }

    let completed = CompletedMultipartUpload::builder()
        .set_parts(Some(completed_parts))
        .build();

    client
        .complete_multipart_upload()
        .bucket(bucket)
        .key(key)
        .upload_id(upload_id)
        .multipart_upload(completed)
        .send()
        .await
        .context("complete_multipart_upload failed")?;

    Ok(())
}

pub async fn put_object_file(
    app: &AppHandle,
    client: &Client,
    bucket: &str,
    key: &str,
    local_path: &Path,
    transfer_id: Option<String>,
    cancel_token: Option<CancellationToken>,
) -> Result<()> {
    let metadata = fs::metadata(local_path)
        .await
        .with_context(|| format!("failed to stat {}", local_path.display()))?;
    let total = metadata.len();
    let file_name = local_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(key)
        .to_string();
    let transfer_id = transfer_id.unwrap_or_else(|| Uuid::new_v4().to_string());

    emit_progress(app, &transfer_id, &file_name, "upload", 0, total, "started");

    let result = if total > MULTIPART_THRESHOLD {
        put_object_multipart(
            app,
            client,
            bucket,
            key,
            local_path,
            &transfer_id,
            &file_name,
            total,
            &cancel_token,
        )
        .await
    } else {
        // For small files, check cancel once before reading
        if is_cancelled(&cancel_token) {
            emit_progress(
                app,
                &transfer_id,
                &file_name,
                "upload",
                0,
                total,
                "cancelled",
            );
            return Err(anyhow!("transfer cancelled"));
        }
        let bytes = fs::read(local_path)
            .await
            .with_context(|| format!("failed to read {}", local_path.display()))?;
        put_object_single(client, bucket, key, bytes).await
    };

    match result {
        Ok(()) => {
            emit_progress(
                app,
                &transfer_id,
                &file_name,
                "upload",
                total,
                total,
                "completed",
            );
            Ok(())
        }
        Err(err) if err.to_string().contains("transfer cancelled") => Err(err),
        Err(err) => {
            emit_progress(
                app,
                &transfer_id,
                &file_name,
                "upload",
                0,
                total,
                "failed",
            );
            Err(err)
        }
    }
}

pub async fn get_object_to_path(
    app: &AppHandle,
    client: &Client,
    bucket: &str,
    key: &str,
    dest_path: &Path,
    transfer_id: Option<String>,
    cancel_token: Option<CancellationToken>,
) -> Result<()> {
    let file_name = Path::new(key)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(key)
        .to_string();
    let transfer_id = transfer_id.unwrap_or_else(|| Uuid::new_v4().to_string());

    emit_progress(
        app,
        &transfer_id,
        &file_name,
        "download",
        0,
        0,
        "started",
    );

    let response = client
        .get_object()
        .bucket(bucket)
        .key(key)
        .send()
        .await
        .context("get_object failed")?;

    let total = response.content_length().unwrap_or(0) as u64;

    if let Some(parent) = dest_path.parent() {
        fs::create_dir_all(parent)
            .await
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let mut file = fs::File::create(dest_path)
        .await
        .with_context(|| format!("failed to create {}", dest_path.display()))?;

    let mut body = response.body;
    let mut downloaded: u64 = 0;

    while let Some(chunk_res) = body.next().await {
        // Cancel check before processing each chunk
        if is_cancelled(&cancel_token) {
            drop(file);
            let _ = fs::remove_file(dest_path).await;
            emit_progress(
                app,
                &transfer_id,
                &file_name,
                "download",
                downloaded,
                total,
                "cancelled",
            );
            return Err(anyhow!("transfer cancelled"));
        }

        // Pause check
        let cancelled_during_pause = wait_if_paused(
            app,
            &transfer_id,
            &file_name,
            "download",
            downloaded,
            total,
            &cancel_token,
        )
        .await;
        if cancelled_during_pause {
            drop(file);
            let _ = fs::remove_file(dest_path).await;
            emit_progress(
                app,
                &transfer_id,
                &file_name,
                "download",
                downloaded,
                total,
                "cancelled",
            );
            return Err(anyhow!("transfer cancelled"));
        }

        let chunk = chunk_res.context("failed to read response body chunk")?;
        file.write_all(&chunk)
            .await
            .context("failed to write to destination file")?;
        downloaded += chunk.len() as u64;
        emit_progress(
            app,
            &transfer_id,
            &file_name,
            "download",
            downloaded,
            total,
            "in_progress",
        );
    }

    file.flush().await.context("failed to flush destination file")?;

    emit_progress(
        app,
        &transfer_id,
        &file_name,
        "download",
        total,
        total,
        "completed",
    );

    Ok(())
}

pub async fn delete_objects_batch(
    client: &Client,
    bucket: &str,
    keys: &[String],
) -> Result<(), PakerError> {
    if keys.is_empty() {
        return Ok(());
    }

    let objects: Vec<ObjectIdentifier> = keys
        .iter()
        .map(|key| {
            ObjectIdentifier::builder()
                .key(key)
                .build()
                .expect("valid object identifier")
        })
        .collect();

    client
        .delete_objects()
        .bucket(bucket)
        .delete(
            aws_sdk_s3::types::Delete::builder()
                .set_objects(Some(objects))
                .build()
                .map_err(|_| PakerError::Internal)?,
        )
        .send()
        .await
        .map_err(map_s3_sdk_error)?;

    Ok(())
}

pub async fn rename_object(
    client: &Client,
    bucket: &str,
    old_key: &str,
    new_key: &str,
) -> Result<(), PakerError> {
    let copy_source = format!("{bucket}/{old_key}");
    client
        .copy_object()
        .bucket(bucket)
        .key(new_key)
        .copy_source(copy_source)
        .send()
        .await
        .map_err(map_s3_sdk_error)?;

    delete_objects_batch(client, bucket, &[old_key.to_string()]).await
}

pub async fn create_folder(
    client: &Client,
    bucket: &str,
    prefix: &str,
    folder_name: &str,
) -> Result<(), PakerError> {
    let mut key = prefix.to_string();
    if !key.is_empty() && !key.ends_with('/') {
        key.push('/');
    }
    key.push_str(folder_name.trim_matches('/'));
    if !key.ends_with('/') {
        key.push('/');
    }

    client
        .put_object()
        .bucket(bucket)
        .key(key)
        .body(ByteStream::from_static(b""))
        .send()
        .await
        .map_err(map_s3_sdk_error)?;

    Ok(())
}

pub fn join_prefix(prefix: &str, file_name: &str) -> String {
    let mut key = prefix.to_string();
    if !key.is_empty() && !key.ends_with('/') {
        key.push('/');
    }
    key.push_str(
        Path::new(file_name)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(file_name),
    );
    key
}

pub fn local_dest_path(save_dir: &Path, key: &str) -> Result<PathBuf> {
    safe_local_dest_path(save_dir, key)
}

fn preview_etag_sidecar(dest: &Path) -> PathBuf {
    PathBuf::from(format!("{}.etag", dest.display()))
}

fn preview_cache_file_name(bucket: &str, key: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bucket.hash(&mut hasher);
    key.hash(&mut hasher);
    let hash = hasher.finish();
    let ext = Path::new(key)
        .extension()
        .and_then(|e| e.to_str())
        .filter(|e| !e.is_empty());
    match ext {
        Some(ext) => format!("{hash:x}.{ext}"),
        None => format!("{hash:x}"),
    }
}

pub async fn preview_object_to_cache(
    app: &AppHandle,
    client: &Client,
    cache_dir: &Path,
    bucket: &str,
    key: &str,
    head: &ObjectHeadResult,
) -> Result<String> {
    let size = head.content_length.unwrap_or(0).max(0) as u64;
    if size > PREVIEW_MAX_BYTES {
        return Err(anyhow!(
            "object too large for preview ({size} bytes, max {PREVIEW_MAX_BYTES})"
        ));
    }

    let dest = cache_dir.join(preview_cache_file_name(bucket, key));
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)
            .await
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let etag_sidecar = preview_etag_sidecar(&dest);
    if dest.is_file() {
        if let Some(remote_etag) = head.etag.as_deref() {
            if let Ok(cached_etag) = fs::read_to_string(&etag_sidecar).await {
                if cached_etag.trim() == remote_etag.trim() {
                    return Ok(dest.to_string_lossy().into_owned());
                }
            }
        }
    }

    get_object_to_path(app, client, bucket, key, &dest, None, None).await?;

    if let Some(etag) = &head.etag {
        let _ = fs::write(&etag_sidecar, etag).await;
    }

    Ok(dest.to_string_lossy().into_owned())
}

pub struct CopyItem {
    pub src_key: String,
    pub dest_key: String,
}

pub async fn copy_objects_batch(
    app: &AppHandle,
    client: &Client,
    src_bucket: &str,
    dest_bucket: &str,
    items: &[CopyItem],
) -> Result<()> {
    for item in items {
        let copy_source = format!("{src_bucket}/{}", item.src_key);
        let transfer_id = Uuid::new_v4().to_string();
        let file_name = Path::new(&item.src_key)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&item.src_key)
            .to_string();

        emit_progress(app, &transfer_id, &file_name, "copy", 0, 0, "started");

        client
            .copy_object()
            .bucket(dest_bucket)
            .key(&item.dest_key)
            .copy_source(copy_source)
            .send()
            .await
            .with_context(|| {
                format!("copy_object failed: {} -> {}", item.src_key, item.dest_key)
            })?;

        emit_progress(app, &transfer_id, &file_name, "copy", 0, 0, "completed");
    }
    Ok(())
}

pub async fn move_objects_batch(
    app: &AppHandle,
    client: &Client,
    src_bucket: &str,
    dest_bucket: &str,
    items: &[CopyItem],
) -> Result<(), PakerError> {
    copy_objects_batch(app, client, src_bucket, dest_bucket, items)
        .await
        .map_err(into_ipc_error)?;
    let src_keys: Vec<String> = items.iter().map(|i| i.src_key.clone()).collect();
    delete_objects_batch(client, src_bucket, &src_keys).await
}
