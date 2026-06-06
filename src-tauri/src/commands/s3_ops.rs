use crate::s3::build_client_for_id;
use crate::s3::operations::{
    create_folder as s3_create_folder, delete_objects_batch, get_object_to_path,
    head_object as s3_head_object, join_prefix, list_buckets as s3_list_buckets, list_objects_v2,
    local_dest_path, object_exists, put_object_file, rename_object as s3_rename_object,
    verify_bucket_access,
};
use crate::s3::{BucketInfo, ListObjectsResult, ObjectHeadResult};
use crate::storage;
use std::path::PathBuf;
use tauri::AppHandle;

fn map_err(err: impl ToString) -> String {
    err.to_string()
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
pub async fn list_objects(
    app: AppHandle,
    connection_id: String,
    bucket: String,
    prefix: Option<String>,
    continuation_token: Option<String>,
) -> Result<ListObjectsResult, String> {
    let client = build_client_for_id(&app, &connection_id)
        .await
        .map_err(map_err)?;
    list_objects_v2(
        &client,
        &bucket,
        prefix.as_deref(),
        continuation_token.as_deref(),
    )
    .await
    .map_err(map_err)
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
    let paths = if local_paths.is_empty() {
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

    let mut uploaded_keys = Vec::new();
    for local_path in paths {
        let path = PathBuf::from(&local_path);
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| format!("invalid file path: {local_path}"))?;
        let key = join_prefix(&prefix, file_name);
        put_object_file(&app, &client, &bucket, &key, &path)
            .await
            .map_err(map_err)?;
        uploaded_keys.push(key);
    }

    Ok(uploaded_keys)
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

    let mut saved_paths = Vec::new();
    for key in keys {
        let dest = local_dest_path(&save_dir, &key);
        get_object_to_path(&app, &client, &bucket, &key, &dest)
            .await
            .map_err(map_err)?;
        saved_paths.push(dest.to_string_lossy().into_owned());
    }

    Ok(saved_paths)
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
        .map_err(map_err)
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
        .map_err(map_err)
}

#[tauri::command]
pub async fn head_object(
    app: AppHandle,
    connection_id: String,
    bucket: String,
    key: String,
) -> Result<ObjectHeadResult, String> {
    let client = build_client_for_id(&app, &connection_id)
        .await
        .map_err(map_err)?;
    s3_head_object(&client, &bucket, &key)
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
    Ok(key)
}
