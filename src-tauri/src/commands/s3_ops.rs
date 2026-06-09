use crate::commands::local_fs::LocalFsScope;
use crate::error::{clamp_presign_expiry_secs, into_ipc_error, PakerError};
use crate::s3::build_client_for_id;
use crate::s3::operations::{
    calculate_prefix_size as s3_calculate_prefix_size, copy_objects_batch as s3_copy_objects_batch,
    create_folder as s3_create_folder, delete_objects_batch,
    get_bucket_metadata as s3_get_bucket_metadata, get_object_to_path,
    head_object as s3_head_object, join_prefix, list_buckets as s3_list_buckets, list_objects_v2,
    local_dest_path, move_objects_batch as s3_move_objects_batch, object_exists,
    object_info_to_head, presign_get_object as s3_presign_get_object,
    preview_object_to_cache as s3_preview_object_to_cache, put_object_file,
    rename_object as s3_rename_object, verify_bucket_access, CopyItem,
};
use crate::s3::{
    BucketInfo, BucketMetadata, CachedListResponse, ListObjectsResponse, ObjectHeadResponse,
    PrefixSizeResult,
};
use crate::storage::{self, ObjectCacheManager};
use crate::transfer::TransferManager;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use uuid::Uuid;

fn timestamp_now() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}

fn invalidate_after_mutation(cache: &ObjectCacheManager, conn: &str, bucket: &str, prefix: &str) {
    let _ = cache.invalidate_prefix(conn, bucket, prefix);
    let _ = cache.invalidate_parent_if_needed(conn, bucket, prefix);
    let _ = cache.mark_bucket_index_stale(conn, bucket);
}

fn parent_prefix_from_key(key: &str) -> String {
    match key.rfind('/') {
        None | Some(0) => String::new(),
        Some(idx) => key[..=idx].to_string(),
    }
}

fn prefixes_for_keys(keys: &[String]) -> Vec<String> {
    let mut prefixes: Vec<String> = keys.iter().map(|k| parent_prefix_from_key(k)).collect();
    prefixes.sort();
    prefixes.dedup();
    prefixes
}

fn invalidate_for_keys(cache: &ObjectCacheManager, conn: &str, bucket: &str, keys: &[String]) {
    for prefix in prefixes_for_keys(keys) {
        invalidate_after_mutation(cache, conn, bucket, &prefix);
    }
}

fn enrich_prefix_last_modified(
    cache: &ObjectCacheManager,
    connection_id: &str,
    bucket: &str,
    listing: &mut crate::s3::ListObjectsResult,
) {
    let index_complete = cache
        .get_bucket_index_meta(connection_id, bucket)
        .is_some_and(|meta| meta.status == "completed");
    if !index_complete {
        listing.prefix_last_modified = HashMap::new();
        return;
    }

    let mut prefix_dates = HashMap::new();
    for prefix in &listing.common_prefixes {
        if listing
            .objects
            .iter()
            .any(|obj| obj.key == *prefix && obj.last_modified.is_some())
        {
            continue;
        }
        if let Some(max_modified) =
            cache.get_prefix_max_last_modified(connection_id, bucket, prefix)
        {
            prefix_dates.insert(prefix.clone(), max_modified);
        }
    }
    listing.prefix_last_modified = prefix_dates;
}

fn seed_head_cache_from_listing(
    cache: &ObjectCacheManager,
    connection_id: &str,
    bucket: &str,
    listing: &crate::s3::ListObjectsResult,
) {
    for obj in &listing.objects {
        if obj.is_prefix || obj.key.ends_with('/') {
            continue;
        }
        let head = object_info_to_head(obj);
        let _ = cache.put_head(connection_id, bucket, &obj.key, &head);
    }
}

fn max_concurrent(app: &AppHandle) -> usize {
    storage::ui_state::get_max_concurrent_transfers(app).max(1) as usize
}

#[tauri::command]
pub async fn list_buckets(
    app: AppHandle,
    connection_id: String,
    force_all: Option<bool>,
) -> Result<Vec<BucketInfo>, PakerError> {
    let profile = storage::get_connection(&app, &connection_id)
        .map_err(into_ipc_error)?
        .ok_or(PakerError::ConnectionNotFound)?;

    if !force_all.unwrap_or(false) {
        if let Some(bucket) = profile
            .default_bucket
            .as_ref()
            .map(|b| b.trim())
            .filter(|b| !b.is_empty())
        {
            return Ok(vec![BucketInfo {
                name: bucket.to_string(),
                creation_date: None,
            }]);
        }
    }

    let client = build_client_for_id(&app, &connection_id).await?;
    s3_list_buckets(&client).await
}

#[tauri::command]
pub async fn verify_bucket(
    app: AppHandle,
    connection_id: String,
    bucket: String,
) -> Result<(), PakerError> {
    let bucket = bucket.trim().to_string();
    if bucket.is_empty() {
        return Err(PakerError::InvalidInput(
            "Bucket name is required".to_string(),
        ));
    }

    let client = build_client_for_id(&app, &connection_id).await?;
    verify_bucket_access(&client, &bucket).await
}

#[tauri::command]
pub async fn calculate_prefix_size(
    app: AppHandle,
    connection_id: String,
    bucket: String,
    prefix: Option<String>,
    force_refresh: Option<bool>,
) -> Result<PrefixSizeResult, PakerError> {
    let prefix_str = prefix.as_deref().unwrap_or("");
    let normalized = if prefix_str.is_empty() {
        String::new()
    } else if prefix_str.ends_with('/') {
        prefix_str.to_string()
    } else {
        format!("{prefix_str}/")
    };

    let cache = app.state::<ObjectCacheManager>();
    if !force_refresh.unwrap_or(false) {
        if let Some((result, _calculated_at)) =
            cache.get_prefix_size(&connection_id, &bucket, &normalized)
        {
            return Ok(result);
        }
    }

    let client = build_client_for_id(&app, &connection_id).await?;
    let result = s3_calculate_prefix_size(&app, &client, &bucket, prefix_str).await?;
    let _ = cache.put_prefix_size(&connection_id, &bucket, &result.prefix, &result);
    Ok(result)
}

#[tauri::command]
pub async fn get_bucket_metadata(
    app: AppHandle,
    connection_id: String,
    bucket: String,
) -> Result<BucketMetadata, PakerError> {
    let profile = storage::get_connection(&app, &connection_id)
        .map_err(into_ipc_error)?
        .ok_or(PakerError::ConnectionNotFound)?;

    let client = build_client_for_id(&app, &connection_id).await?;

    let creation_date = s3_list_buckets(&client).await.ok().and_then(|buckets| {
        buckets
            .into_iter()
            .find(|b| b.name == bucket)
            .and_then(|b| b.creation_date)
    });

    s3_get_bucket_metadata(
        &client,
        &bucket,
        creation_date,
        Some(profile.name),
        profile.endpoint,
        Some(profile.region),
        Some(profile.force_path_style),
    )
    .await
}

#[tauri::command]
pub async fn read_list_cache(
    app: AppHandle,
    connection_id: String,
    bucket: String,
    prefix: Option<String>,
) -> Result<Option<CachedListResponse>, PakerError> {
    let cache = app.state::<ObjectCacheManager>();
    let prefix_str = prefix.as_deref().unwrap_or("");
    Ok(cache
        .get_listing(&connection_id, &bucket, prefix_str, "")
        .map(|(mut result, fetched_at)| {
            enrich_prefix_last_modified(&cache, &connection_id, &bucket, &mut result);
            CachedListResponse { result, fetched_at }
        }))
}

#[tauri::command]
pub async fn list_objects(
    app: AppHandle,
    connection_id: String,
    bucket: String,
    prefix: Option<String>,
    continuation_token: Option<String>,
    force_refresh: Option<bool>,
) -> Result<ListObjectsResponse, PakerError> {
    let prefix_str = prefix.as_deref().unwrap_or("");
    let token_str = continuation_token.as_deref().unwrap_or("");
    let cache = app.state::<ObjectCacheManager>();

    // `read_list_cache` serves stale-while-revalidate; this command always hits S3 so the
    // UI can show cached data instantly then replace it with a fresh listing.
    if force_refresh.unwrap_or(false) && token_str.is_empty() {
        let _ = cache.invalidate_prefix(&connection_id, &bucket, prefix_str);
    }

    let client = build_client_for_id(&app, &connection_id).await?;
    let mut result = list_objects_v2(
        &client,
        &bucket,
        prefix.as_deref(),
        continuation_token.as_deref(),
    )
    .await?;

    enrich_prefix_last_modified(&cache, &connection_id, &bucket, &mut result);

    let _ = cache.put_listing(&connection_id, &bucket, prefix_str, token_str, &result);
    seed_head_cache_from_listing(&cache, &connection_id, &bucket, &result);

    Ok(ListObjectsResponse {
        result,
        from_cache: false,
        fetched_at: Some(timestamp_now()),
    })
}

#[tauri::command]
pub async fn pick_upload_files(app: AppHandle) -> Result<Vec<String>, PakerError> {
    let picked: Vec<PathBuf> = rfd::FileDialog::new()
        .set_title("Select files to upload")
        .pick_files()
        .unwrap_or_default();

    let scope = app.state::<LocalFsScope>();
    scope.register_file_paths(&picked);

    Ok(picked
        .into_iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect())
}

#[tauri::command]
pub async fn upload_files(
    app: AppHandle,
    connection_id: String,
    bucket: String,
    prefix: String,
    local_paths: Vec<String>,
) -> Result<Vec<String>, PakerError> {
    let scope = app.state::<LocalFsScope>();
    let validated_paths: Vec<PathBuf> = if local_paths.is_empty() {
        let picked: Vec<PathBuf> = rfd::FileDialog::new()
            .set_title("Select files to upload")
            .pick_files()
            .unwrap_or_default();
        scope.register_file_paths(&picked);
        picked
            .iter()
            .map(|p| scope.validate_file_access(p.as_path()))
            .collect::<Result<Vec<_>, _>>()?
    } else {
        let path_bufs: Vec<PathBuf> = local_paths.into_iter().map(PathBuf::from).collect();
        scope.prepare_upload_paths(&path_bufs)?
    };

    if validated_paths.is_empty() {
        return Ok(Vec::new());
    }

    let client = build_client_for_id(&app, &connection_id).await?;

    let sem = Arc::new(Semaphore::new(max_concurrent(&app)));
    let mut set: JoinSet<Result<String, PakerError>> = JoinSet::new();

    for path in validated_paths {
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        let key = join_prefix(&prefix, &file_name);
        let transfer_id = Uuid::new_v4().to_string();

        // Register before spawning so cancel commands can target it immediately
        let token = app.state::<TransferManager>().register(&transfer_id);

        let permit = Arc::clone(&sem)
            .acquire_owned()
            .await
            .map_err(|_| PakerError::Internal)?;
        let app_clone = app.clone();
        let client_clone = client.clone();
        let bucket_clone = bucket.clone();
        let key_clone = key.clone();
        let tid = transfer_id.clone();

        set.spawn(async move {
            let _permit = permit;
            let result = put_object_file(
                &app_clone,
                &client_clone,
                &bucket_clone,
                &key_clone,
                &path,
                Some(tid.clone()),
                Some(token),
            )
            .await;
            app_clone.state::<TransferManager>().remove(&tid);
            result.map(|_| key_clone).map_err(into_ipc_error)
        });
    }

    let results = collect_results(set).await?;
    let cache = app.state::<ObjectCacheManager>();
    invalidate_after_mutation(&cache, &connection_id, &bucket, &prefix);
    Ok(results)
}

#[tauri::command]
pub async fn download_files(
    app: AppHandle,
    connection_id: String,
    bucket: String,
    keys: Vec<String>,
    save_dir: Option<String>,
) -> Result<Vec<String>, PakerError> {
    if keys.is_empty() {
        return Ok(Vec::new());
    }

    let scope = app.state::<LocalFsScope>();
    let save_dir = match save_dir.filter(|d| !d.is_empty()) {
        Some(dir) => scope.validate_dir_access(Path::new(&dir))?,
        None => {
            let picked = rfd::FileDialog::new()
                .set_title("Select download folder")
                .pick_folder()
                .ok_or_else(|| {
                    PakerError::InvalidInput("Download cancelled: no folder selected".to_string())
                })?;
            scope.register_picked_folder(&picked);
            scope.validate_dir_access(&picked)?
        }
    };

    let client = build_client_for_id(&app, &connection_id).await?;

    let sem = Arc::new(Semaphore::new(max_concurrent(&app)));
    let mut set: JoinSet<Result<String, PakerError>> = JoinSet::new();

    for key in keys {
        let dest = local_dest_path(&save_dir, &key).map_err(|_| {
            PakerError::InvalidInput("Object key is not a valid local file path".to_string())
        })?;
        let transfer_id = Uuid::new_v4().to_string();

        let token = app.state::<TransferManager>().register(&transfer_id);

        let permit = Arc::clone(&sem)
            .acquire_owned()
            .await
            .map_err(|_| PakerError::Internal)?;
        let app_clone = app.clone();
        let client_clone = client.clone();
        let bucket_clone = bucket.clone();
        let tid = transfer_id.clone();

        set.spawn(async move {
            let _permit = permit;
            let result = get_object_to_path(
                &app_clone,
                &client_clone,
                &bucket_clone,
                &key,
                &dest,
                Some(tid.clone()),
                Some(token),
            )
            .await;
            app_clone.state::<TransferManager>().remove(&tid);
            result
                .map(|_| dest.to_string_lossy().into_owned())
                .map_err(into_ipc_error)
        });
    }

    collect_results(set).await
}

async fn collect_results(
    mut set: JoinSet<Result<String, PakerError>>,
) -> Result<Vec<String>, PakerError> {
    let mut results = Vec::new();
    let mut first_error: Option<PakerError> = None;

    while let Some(join_result) = set.join_next().await {
        match join_result {
            Ok(Ok(value)) => results.push(value),
            Ok(Err(e)) => {
                if first_error.is_none() {
                    first_error = Some(e);
                }
            }
            Err(_) => {
                if first_error.is_none() {
                    first_error = Some(PakerError::Internal);
                }
            }
        }
    }

    if let Some(err) = first_error {
        return Err(err);
    }
    Ok(results)
}

#[tauri::command]
pub async fn cancel_transfer(app: AppHandle, transfer_id: String) -> Result<(), PakerError> {
    app.state::<TransferManager>().cancel(&transfer_id);
    Ok(())
}

#[tauri::command]
pub async fn pause_transfer(app: AppHandle, transfer_id: String) -> Result<(), PakerError> {
    app.state::<TransferManager>().pause(&transfer_id);
    Ok(())
}

#[tauri::command]
pub async fn resume_transfer(app: AppHandle, transfer_id: String) -> Result<(), PakerError> {
    app.state::<TransferManager>().resume(&transfer_id);
    Ok(())
}

#[tauri::command]
pub async fn delete_objects(
    app: AppHandle,
    connection_id: String,
    bucket: String,
    keys: Vec<String>,
) -> Result<(), PakerError> {
    let client = build_client_for_id(&app, &connection_id).await?;
    delete_objects_batch(&client, &bucket, &keys).await?;

    let cache = app.state::<ObjectCacheManager>();
    invalidate_for_keys(&cache, &connection_id, &bucket, &keys);
    Ok(())
}

#[tauri::command]
pub async fn rename_object(
    app: AppHandle,
    connection_id: String,
    bucket: String,
    old_key: String,
    new_key: String,
) -> Result<(), PakerError> {
    let client = build_client_for_id(&app, &connection_id).await?;
    s3_rename_object(&client, &bucket, &old_key, &new_key).await?;

    let cache = app.state::<ObjectCacheManager>();
    invalidate_after_mutation(
        &cache,
        &connection_id,
        &bucket,
        &parent_prefix_from_key(&old_key),
    );
    invalidate_after_mutation(
        &cache,
        &connection_id,
        &bucket,
        &parent_prefix_from_key(&new_key),
    );
    let _ = cache.invalidate_head(&connection_id, &bucket, &old_key);
    let _ = cache.invalidate_head(&connection_id, &bucket, &new_key);
    Ok(())
}

#[tauri::command]
pub async fn head_object(
    app: AppHandle,
    connection_id: String,
    bucket: String,
    key: String,
    force_refresh: Option<bool>,
) -> Result<ObjectHeadResponse, PakerError> {
    let cache = app.state::<ObjectCacheManager>();

    if !force_refresh.unwrap_or(false) {
        if let Some((result, fetched_at)) = cache.get_head(&connection_id, &bucket, &key) {
            return Ok(ObjectHeadResponse {
                result,
                from_cache: true,
                fetched_at: Some(fetched_at),
            });
        }
    }

    let client = build_client_for_id(&app, &connection_id).await?;
    let result = s3_head_object(&client, &bucket, &key).await?;
    let _ = cache.put_head(&connection_id, &bucket, &key, &result);

    Ok(ObjectHeadResponse {
        result,
        from_cache: false,
        fetched_at: Some(timestamp_now()),
    })
}

#[tauri::command]
pub async fn presign_object(
    app: AppHandle,
    connection_id: String,
    bucket: String,
    key: String,
    expires_secs: Option<u64>,
) -> Result<String, PakerError> {
    let client = build_client_for_id(&app, &connection_id).await?;
    let expires = clamp_presign_expiry_secs(expires_secs.unwrap_or(3600));
    s3_presign_get_object(&client, &bucket, &key, expires).await
}

#[tauri::command]
pub async fn preview_object_to_cache(
    app: AppHandle,
    connection_id: String,
    bucket: String,
    key: String,
) -> Result<String, PakerError> {
    let client = build_client_for_id(&app, &connection_id).await?;
    let cache = app.state::<ObjectCacheManager>();
    let head = if let Some((result, _)) = cache.get_head(&connection_id, &bucket, &key) {
        result
    } else {
        let fetched = s3_head_object(&client, &bucket, &key).await?;
        let _ = cache.put_head(&connection_id, &bucket, &key, &fetched);
        fetched
    };

    let cache_dir = storage::paths::preview_cache_dir(&app).map_err(into_ipc_error)?;
    s3_preview_object_to_cache(&app, &client, &cache_dir, &bucket, &key, &head)
        .await
        .map_err(into_ipc_error)
}

#[tauri::command]
pub async fn check_objects_exist(
    app: AppHandle,
    connection_id: String,
    bucket: String,
    keys: Vec<String>,
) -> Result<Vec<String>, PakerError> {
    if keys.is_empty() {
        return Ok(Vec::new());
    }

    let client = build_client_for_id(&app, &connection_id).await?;

    let mut existing = Vec::new();
    for key in keys {
        if object_exists(&client, &bucket, &key).await? {
            existing.push(key);
        }
    }

    Ok(existing)
}

#[tauri::command]
pub async fn create_folder(
    app: AppHandle,
    connection_id: String,
    bucket: String,
    prefix: String,
    folder_name: String,
) -> Result<String, PakerError> {
    let client = build_client_for_id(&app, &connection_id).await?;
    s3_create_folder(&client, &bucket, &prefix, &folder_name).await?;

    let mut key = prefix;
    if !key.is_empty() && !key.ends_with('/') {
        key.push('/');
    }
    key.push_str(folder_name.trim_matches('/'));
    if !key.ends_with('/') {
        key.push('/');
    }

    let cache = app.state::<ObjectCacheManager>();
    invalidate_after_mutation(&cache, &connection_id, &bucket, &key);
    Ok(key)
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyMoveItem {
    pub src_key: String,
    pub dest_key: Option<String>,
}

fn resolve_dest_key(src_key: &str, dest_key: Option<&str>, dest_prefix: Option<&str>) -> String {
    if let Some(key) = dest_key {
        return key.to_string();
    }
    let file_name = Path::new(src_key)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(src_key);
    match dest_prefix {
        Some(prefix) if !prefix.is_empty() => {
            let mut key = prefix.to_string();
            if !key.ends_with('/') {
                key.push('/');
            }
            key.push_str(file_name);
            key
        }
        _ => file_name.to_string(),
    }
}

fn resolve_items(items: Vec<CopyMoveItem>, dest_prefix: Option<&str>) -> Vec<CopyItem> {
    items
        .into_iter()
        .map(|item| {
            let dest_key = resolve_dest_key(&item.src_key, item.dest_key.as_deref(), dest_prefix);
            CopyItem {
                src_key: item.src_key,
                dest_key,
            }
        })
        .collect()
}

#[tauri::command]
pub async fn copy_objects(
    app: AppHandle,
    connection_id: String,
    src_bucket: String,
    dest_bucket: String,
    items: Vec<CopyMoveItem>,
    dest_prefix: Option<String>,
) -> Result<(), PakerError> {
    if items.is_empty() {
        return Ok(());
    }
    let client = build_client_for_id(&app, &connection_id).await?;
    let resolved = resolve_items(items, dest_prefix.as_deref());
    s3_copy_objects_batch(&app, &client, &src_bucket, &dest_bucket, &resolved)
        .await
        .map_err(into_ipc_error)?;

    let cache = app.state::<ObjectCacheManager>();
    let src_keys: Vec<String> = resolved.iter().map(|i| i.src_key.clone()).collect();
    let dest_keys: Vec<String> = resolved.iter().map(|i| i.dest_key.clone()).collect();
    invalidate_for_keys(&cache, &connection_id, &src_bucket, &src_keys);
    invalidate_for_keys(&cache, &connection_id, &dest_bucket, &dest_keys);
    if let Some(prefix) = dest_prefix.as_deref().filter(|p| !p.is_empty()) {
        invalidate_after_mutation(&cache, &connection_id, &dest_bucket, prefix);
    }
    Ok(())
}

#[tauri::command]
pub async fn move_objects(
    app: AppHandle,
    connection_id: String,
    src_bucket: String,
    dest_bucket: String,
    items: Vec<CopyMoveItem>,
    dest_prefix: Option<String>,
) -> Result<(), PakerError> {
    if items.is_empty() {
        return Ok(());
    }
    let client = build_client_for_id(&app, &connection_id).await?;
    let resolved = resolve_items(items, dest_prefix.as_deref());
    s3_move_objects_batch(&app, &client, &src_bucket, &dest_bucket, &resolved).await?;

    let cache = app.state::<ObjectCacheManager>();
    let src_keys: Vec<String> = resolved.iter().map(|i| i.src_key.clone()).collect();
    let dest_keys: Vec<String> = resolved.iter().map(|i| i.dest_key.clone()).collect();
    invalidate_for_keys(&cache, &connection_id, &src_bucket, &src_keys);
    invalidate_for_keys(&cache, &connection_id, &dest_bucket, &dest_keys);
    if let Some(prefix) = dest_prefix.as_deref().filter(|p| !p.is_empty()) {
        invalidate_after_mutation(&cache, &connection_id, &dest_bucket, prefix);
    }
    Ok(())
}
