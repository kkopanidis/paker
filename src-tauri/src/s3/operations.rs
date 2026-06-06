use anyhow::{anyhow, Context, Result};
use aws_sdk_s3::error::SdkError;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::types::{CompletedMultipartUpload, CompletedPart, ObjectIdentifier};
use aws_sdk_s3::Client;
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};
use tokio::fs;
use tokio::io::AsyncReadExt;
use uuid::Uuid;

const MULTIPART_THRESHOLD: u64 = 5 * 1024 * 1024;
const PART_SIZE: usize = 5 * 1024 * 1024;

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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectInfo {
    pub key: String,
    pub size: i64,
    pub last_modified: Option<String>,
    pub etag: Option<String>,
    pub is_prefix: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListObjectsResult {
    pub objects: Vec<ObjectInfo>,
    pub common_prefixes: Vec<String>,
    pub continuation_token: Option<String>,
    pub is_truncated: bool,
}

#[derive(Debug, Clone, Serialize)]
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

pub async fn list_buckets(client: &Client) -> Result<Vec<BucketInfo>> {
    let response = client.list_buckets().send().await.context("list_buckets failed")?;

    Ok(response
        .buckets()
        .iter()
        .filter_map(|bucket| {
            bucket.name().map(|name| BucketInfo {
                name: name.to_string(),
                creation_date: bucket
                    .creation_date()
                    .map(|dt| dt.to_string()),
            })
        })
        .collect())
}

pub async fn head_object(client: &Client, bucket: &str, key: &str) -> Result<ObjectHeadResult> {
    let response = client
        .head_object()
        .bucket(bucket)
        .key(key)
        .send()
        .await
        .context("head_object failed")?;

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

pub async fn object_exists(client: &Client, bucket: &str, key: &str) -> Result<bool> {
    match client.head_object().bucket(bucket).key(key).send().await {
        Ok(_) => Ok(true),
        Err(SdkError::ServiceError(err)) if err.err().is_not_found() => Ok(false),
        Err(err) => Err(anyhow!("head_object failed: {err}")),
    }
}

pub async fn verify_bucket_access(client: &Client, bucket: &str) -> Result<()> {
    match client.head_bucket().bucket(bucket).send().await {
        Ok(_) => return Ok(()),
        Err(head_err) => {
            client
                .list_objects_v2()
                .bucket(bucket)
                .max_keys(1)
                .send()
                .await
                .with_context(|| format!("bucket access check failed for '{bucket}': {head_err}"))?;
        }
    }
    Ok(())
}

pub async fn list_objects_v2(
    client: &Client,
    bucket: &str,
    prefix: Option<&str>,
    continuation_token: Option<&str>,
) -> Result<ListObjectsResult> {
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

    let response = request.send().await.context("list_objects_v2 failed")?;

    let objects = response
        .contents()
        .iter()
        .filter_map(|object| {
            object.key().map(|key| ObjectInfo {
                key: key.to_string(),
                size: object.size().unwrap_or_default(),
                last_modified: object.last_modified().map(|dt| dt.to_string()),
                etag: object.e_tag().map(|etag| etag.to_string()),
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

async fn put_object_multipart(
    app: &AppHandle,
    client: &Client,
    bucket: &str,
    key: &str,
    path: &Path,
    transfer_id: &str,
    file_name: &str,
    total: u64,
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
    let transfer_id = Uuid::new_v4().to_string();

    emit_progress(app, &transfer_id, &file_name, "upload", 0, total, "started");

    let result = if total > MULTIPART_THRESHOLD {
        put_object_multipart(app, client, bucket, key, local_path, &transfer_id, &file_name, total)
            .await
    } else {
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
) -> Result<()> {
    let file_name = Path::new(key)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(key)
        .to_string();
    let transfer_id = Uuid::new_v4().to_string();

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
    let body = response
        .body
        .collect()
        .await
        .context("failed to read object body")?
        .into_bytes();

    emit_progress(
        app,
        &transfer_id,
        &file_name,
        "download",
        total,
        total,
        "in_progress",
    );

    if let Some(parent) = dest_path.parent() {
        fs::create_dir_all(parent)
            .await
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    fs::write(dest_path, &body)
        .await
        .with_context(|| format!("failed to write {}", dest_path.display()))?;

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

pub async fn delete_objects_batch(client: &Client, bucket: &str, keys: &[String]) -> Result<()> {
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
                .context("failed to build delete request")?,
        )
        .send()
        .await
        .context("delete_objects failed")?;

    Ok(())
}

pub async fn rename_object(
    client: &Client,
    bucket: &str,
    old_key: &str,
    new_key: &str,
) -> Result<()> {
    let copy_source = format!("{bucket}/{old_key}");
    client
        .copy_object()
        .bucket(bucket)
        .key(new_key)
        .copy_source(copy_source)
        .send()
        .await
        .context("copy_object failed")?;

    delete_objects_batch(client, bucket, &[old_key.to_string()]).await
}

pub async fn create_folder(
    client: &Client,
    bucket: &str,
    prefix: &str,
    folder_name: &str,
) -> Result<()> {
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
        .context("create_folder put_object failed")?;

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

pub fn local_dest_path(save_dir: &Path, key: &str) -> PathBuf {
    save_dir.join(key)
}
