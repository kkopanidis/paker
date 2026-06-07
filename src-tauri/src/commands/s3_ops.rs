use crate::s3::build_client_for_id;
use crate::s3::operations::{
    calculate_prefix_size as s3_calculate_prefix_size,
    copy_objects_batch as s3_copy_objects_batch, create_folder as s3_create_folder,
    delete_objects_batch, get_bucket_metadata as s3_get_bucket_metadata, get_object_to_path,
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
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use uuid::Uuid;

fn map_err(err: impl ToString) -> String {
    err.to_string()
}

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
    storage::ui_state::get_max_concurrent_transfers(app)
        .max(1) as usize
}

#[tauri::command]
pub async fn list_buckets(
    app: AppHandle,
    connection_id: String,
    force_all: Option<bool>,
) -> Result<Vec<BucketInfo>, String> {
    let profile = storage::get_connection(&app, &connection_id)
        .map_err(map_err)?
        .ok_or_else(|| format!("connection not found: {connection_id}"))?;

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

    let client = build_client_for_id(&app, &connection_id)
        .await
        .map_err(map_err)?;
    s3_list_buckets(&client).await.map_err(map_err)
}

#[tauri::command]
pub async fn verify_bucket(
    app: AppHandle,
    connection_id: String,
    bucket: String,
) -> Result<(), String> {
    let bucket = bucket.trim().to_string();
    if bucket.is_empty() {
        return Err("bucket name is required".to_string());
    }

    let client = build_client_for_id(&app, &connection_id)
        .await
        .map_err(map_err)?;
    verify_bucket_access(&client, &bucket)
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn calculate_prefix_size(
    app: AppHandle,
    connection_id: String,
    bucket: String,
    prefix: Option<String>,
    force_refresh: Option<bool>,
) -> Result<PrefixSizeResult, String> {
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

    let client = build_client_for_id(&app, &connection_id)
        .await
        .map_err(map_err)?;
    let result =
        s3_calculate_prefix_size(&app, &client, &bucket, prefix_str).await.map_err(map_err)?;
    let _ = cache.put_prefix_size(&connection_id, &bucket, &result.prefix, &result);
    Ok(result)
}

#[tauri::command]
pub async fn get_bucket_metadata(
    app: AppHandle,
    connection_id: String,
    bucket: String,
) -> Result<BucketMetadata, String> {
    let profile = storage::get_connection(&app, &connection_id)
        .map_err(map_err)?
        .ok_or_else(|| format!("connection not found: {connection_id}"))?;

    let client = build_client_for_id(&app, &connection_id)
        .await
        .map_err(map_err)?;

    let creation_date = s3_list_buckets(&client)
        .await
        .ok()
        .and_then(|buckets| {
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
    .map_err(map_err)
}

#[tauri::command]
pub async fn read_list_cache(
    app: AppHandle,
    connection_id: String,
    bucket: String,
    prefix: Option<String>,
) -> Result<Option<CachedListResponse>, String> {
    let cache = app.state::<ObjectCacheManager>();
    let prefix_str = prefix.as_deref().unwrap_or("");
    Ok(cache
        .get_listing(&connection_id, &bucket, prefix_str, "")
        .map(|(result, fetched_at)| CachedListResponse {
            result,
            fetched_at,
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
) -> Result<ListObjectsResponse, String> {
    let prefix_str = prefix.as_deref().unwrap_or("");
    let token_str = continuation_token.as_deref().unwrap_or("");
    let cache = app.state::<ObjectCacheManager>();

    // `read_list_cache` serves stale-while-revalidate; this command always hits S3 so the
    // UI can show cached data instantly then replace it with a fresh listing.
    if force_refresh.unwrap_or(false) && token_str.is_empty() {
        let _ = cache.invalidate_prefix(&connection_id, &bucket, prefix_str);
    }

    let client = build_client_for_id(&app, &connection_id)
        .await
        .map_err(map_err)?;
    let result = list_objects_v2(
        &client,
        &bucket,
        prefix.as_deref(),
        continuation_token.as_deref(),
    )
    .await
    .map_err(map_err)?;

    let _ = cache.put_listing(
        &connection_id,
        &bucket,
        prefix_str,
        token_str,
        &result,
    );
    seed_head_cache_from_listing(&cache, &connection_id, &bucket, &result);

    Ok(ListObjectsResponse {
        result,
        from_cache: false,
        fetched_at: Some(timestamp_now()),
    })
}

#[tauri::command]
pub async fn pick_upload_files() -> Result<Vec<String>, String> {
    Ok(rfd::FileDialog::new()
        .set_title("Select files to upload")
        .pick_files()
        .unwrap_or_default()
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
) -> Result<Vec<String>, String> {
    let paths: Vec<String> = if local_paths.is_empty() {
        rfd::FileDialog::new()
            .set_title("Select files to upload")
            .pick_files()
            .unwrap_or_default()
            .into_iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect()
    } else {
        local_paths
    };

    if paths.is_empty() {
        return Ok(Vec::new());
    }

    let client = build_client_for_id(&app, &connection_id)
        .await
        .map_err(map_err)?;

    let sem = Arc::new(Semaphore::new(max_concurrent(&app)));
    let mut set: JoinSet<Result<String, String>> = JoinSet::new();

    for local_path in paths {
        let path = PathBuf::from(&local_path);
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
            .map_err(|e| e.to_string())?;
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
            result.map(|_| key_clone).map_err(|e| e.to_string())
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
) -> Result<Vec<String>, String> {
    if keys.is_empty() {
        return Ok(Vec::new());
    }

    let save_dir = match save_dir {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => rfd::FileDialog::new()
            .set_title("Select download folder")
            .pick_folder()
            .ok_or_else(|| "download cancelled: no folder selected".to_string())?,
    };

    let client = build_client_for_id(&app, &connection_id)
        .await
        .map_err(map_err)?;

    let sem = Arc::new(Semaphore::new(max_concurrent(&app)));
    let mut set: JoinSet<Result<String, String>> = JoinSet::new();

    for key in keys {
        let dest = local_dest_path(&save_dir, &key);
        let transfer_id = Uuid::new_v4().to_string();

        let token = app.state::<TransferManager>().register(&transfer_id);

        let permit = Arc::clone(&sem)
            .acquire_owned()
            .await
            .map_err(|e| e.to_string())?;
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
                .map_err(|e| e.to_string())
        });
    }

    collect_results(set).await
}

async fn collect_results(mut set: JoinSet<Result<String, String>>) -> Result<Vec<String>, String> {
    let mut results = Vec::new();
    let mut first_error: Option<String> = None;

    while let Some(join_result) = set.join_next().await {
        match join_result {
            Ok(Ok(value)) => results.push(value),
            Ok(Err(e)) => {
                if first_error.is_none() {
                    first_error = Some(e);
                }
            }
            Err(e) => {
                if first_error.is_none() {
                    first_error = Some(e.to_string());
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
pub async fn cancel_transfer(
    app: AppHandle,
    transfer_id: String,
) -> Result<(), String> {
    app.state::<TransferManager>().cancel(&transfer_id);
    Ok(())
}

#[tauri::command]
pub async fn pause_transfer(
    app: AppHandle,
    transfer_id: String,
) -> Result<(), String> {
    app.state::<TransferManager>().pause(&transfer_id);
    Ok(())
}

#[tauri::command]
pub async fn resume_transfer(
    app: AppHandle,
    transfer_id: String,
) -> Result<(), String> {
    app.state::<TransferManager>().resume(&transfer_id);
    Ok(())
}

#[tauri::command]
pub async fn delete_objects(
    app: AppHandle,
    connection_id: String,
    bucket: String,
    keys: Vec<String>,
) -> Result<(), String> {
    let client = build_client_for_id(&app, &connection_id)
        .await
        .map_err(map_err)?;
    delete_objects_batch(&client, &bucket, &keys)
        .await
        .map_err(map_err)?;

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
) -> Result<(), String> {
    let client = build_client_for_id(&app, &connection_id)
        .await
        .map_err(map_err)?;
    s3_rename_object(&client, &bucket, &old_key, &new_key)
        .await
        .map_err(map_err)?;

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
) -> Result<ObjectHeadResponse, String> {
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

    let client = build_client_for_id(&app, &connection_id)
        .await
        .map_err(map_err)?;
    let result = s3_head_object(&client, &bucket, &key)
        .await
        .map_err(map_err)?;
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
) -> Result<String, String> {
    let client = build_client_for_id(&app, &connection_id)
        .await
        .map_err(map_err)?;
    s3_presign_get_object(&client, &bucket, &key, expires_secs.unwrap_or(3600))
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn preview_object_to_cache(
    app: AppHandle,
    connection_id: String,
    bucket: String,
    key: String,
) -> Result<String, String> {
    let client = build_client_for_id(&app, &connection_id)
        .await
        .map_err(map_err)?;
    let cache = app.state::<ObjectCacheManager>();
    let head = if let Some((result, _)) = cache.get_head(&connection_id, &bucket, &key) {
        result
    } else {
        let fetched = s3_head_object(&client, &bucket, &key)
            .await
            .map_err(map_err)?;
        let _ = cache.put_head(&connection_id, &bucket, &key, &fetched);
        fetched
    };

    let cache_dir = storage::paths::preview_cache_dir(&app).map_err(map_err)?;
    s3_preview_object_to_cache(&app, &client, &cache_dir, &bucket, &key, &head)
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn check_objects_exist(
    app: AppHandle,
    connection_id: String,
    bucket: String,
    keys: Vec<String>,
) -> Result<Vec<String>, String> {
    if keys.is_empty() {
        return Ok(Vec::new());
    }

    let client = build_client_for_id(&app, &connection_id)
        .await
        .map_err(map_err)?;

    let mut existing = Vec::new();
    for key in keys {
        if object_exists(&client, &bucket, &key)
            .await
            .map_err(map_err)?
        {
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
) -> Result<String, String> {
    let client = build_client_for_id(&app, &connection_id)
        .await
        .map_err(map_err)?;
    s3_create_folder(&client, &bucket, &prefix, &folder_name)
        .await
        .map_err(map_err)?;

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
            let dest_key =
                resolve_dest_key(&item.src_key, item.dest_key.as_deref(), dest_prefix);
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
) -> Result<(), String> {
    if items.is_empty() {
        return Ok(());
    }
    let client = build_client_for_id(&app, &connection_id)
        .await
        .map_err(map_err)?;
    let resolved = resolve_items(items, dest_prefix.as_deref());
    s3_copy_objects_batch(&app, &client, &src_bucket, &dest_bucket, &resolved)
        .await
        .map_err(map_err)?;

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
) -> Result<(), String> {
    if items.is_empty() {
        return Ok(());
    }
    let client = build_client_for_id(&app, &connection_id)
        .await
        .map_err(map_err)?;
    let resolved = resolve_items(items, dest_prefix.as_deref());
    s3_move_objects_batch(&app, &client, &src_bucket, &dest_bucket, &resolved)
        .await
        .map_err(map_err)?;

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
