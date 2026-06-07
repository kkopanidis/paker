use crate::commands::local_fs::LocalFsScope;
use crate::error::{into_ipc_error, PakerError};
use crate::index::BucketIndexManager;
use crate::s3::build_client_for_id;
use crate::s3::operations::index_bucket_flat;
use crate::storage::{
    bucket_index_job_id, BucketIndexMeta, BucketIndexProgress, IndexedObject, ObjectCacheManager,
};
use crate::transfer::TransferManager;
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager};

#[tauri::command]
pub async fn get_bucket_index_status(
    app: AppHandle,
    connection_id: String,
    bucket: String,
) -> Result<Option<BucketIndexMeta>, PakerError> {
    let cache = app.state::<ObjectCacheManager>();
    let manager = app.state::<BucketIndexManager>();
    let job_id = bucket_index_job_id(&connection_id, &bucket);

    let mut meta = cache
        .get_bucket_index_meta(&connection_id, &bucket)
        .unwrap_or(BucketIndexMeta {
            connection_id: connection_id.clone(),
            bucket: bucket.clone(),
            status: "idle".to_string(),
            object_count: 0,
            started_at: None,
            completed_at: None,
            error: None,
        });

    if manager.is_running(&job_id) {
        meta.status = if app.state::<TransferManager>().is_paused(&job_id) {
            "paused".to_string()
        } else {
            "running".to_string()
        };
    }

    if meta.status == "idle" && meta.object_count == 0 && meta.started_at.is_none() {
        return Ok(None);
    }

    Ok(Some(meta))
}

#[tauri::command]
pub async fn start_bucket_index(
    app: AppHandle,
    connection_id: String,
    bucket: String,
    rebuild: Option<bool>,
) -> Result<String, PakerError> {
    let job_id = bucket_index_job_id(&connection_id, &bucket);
    let manager = app.state::<BucketIndexManager>();

    if !manager.try_start(&job_id) {
        return Err(PakerError::IndexNotReady);
    }

    let client = match build_client_for_id(&app, &connection_id).await {
        Ok(client) => client,
        Err(err) => {
            manager.finish(&job_id);
            return Err(err);
        }
    };

    let token = app.state::<TransferManager>().register(&job_id);
    let rebuild = rebuild.unwrap_or(true);
    let app_clone = app.clone();
    let connection_id_clone = connection_id.clone();
    let bucket_clone = bucket.clone();
    let job_id_clone = job_id.clone();

    tokio::spawn(async move {
        let cache = app_clone.state::<ObjectCacheManager>();
        let result = index_bucket_flat(
            &app_clone,
            &client,
            &cache,
            &connection_id_clone,
            &bucket_clone,
            &job_id_clone,
            token.clone(),
            rebuild,
        )
        .await;

        if let Err(err) = result {
            tracing::warn!(error = %err, "bucket index failed");
            let _ = app_clone.emit(
                "bucket-index-progress",
                BucketIndexProgress {
                    connection_id: connection_id_clone.clone(),
                    bucket: bucket_clone.clone(),
                    object_count: cache
                        .get_bucket_index_meta(&connection_id_clone, &bucket_clone)
                        .map(|m| m.object_count)
                        .unwrap_or(0),
                    status: "failed".to_string(),
                    done: true,
                    error: Some("Indexing failed".to_string()),
                },
            );
        }

        app_clone.state::<TransferManager>().remove(&job_id_clone);
        app_clone
            .state::<BucketIndexManager>()
            .finish(&job_id_clone);
    });

    Ok(job_id)
}

#[tauri::command]
pub async fn pause_bucket_index(
    app: AppHandle,
    connection_id: String,
    bucket: String,
) -> Result<(), PakerError> {
    let job_id = bucket_index_job_id(&connection_id, &bucket);
    if !app.state::<BucketIndexManager>().is_running(&job_id) {
        return Err(PakerError::IndexNotReady);
    }
    app.state::<TransferManager>().pause(&job_id);

    let cache = app.state::<ObjectCacheManager>();
    if let Some(mut meta) = cache.get_bucket_index_meta(&connection_id, &bucket) {
        meta.status = "paused".to_string();
        let _ = cache.upsert_bucket_index_meta(&meta);
    }

    Ok(())
}

#[tauri::command]
pub async fn resume_bucket_index(
    app: AppHandle,
    connection_id: String,
    bucket: String,
) -> Result<(), PakerError> {
    let job_id = bucket_index_job_id(&connection_id, &bucket);
    if !app.state::<BucketIndexManager>().is_running(&job_id) {
        return Err(PakerError::IndexNotReady);
    }
    app.state::<TransferManager>().resume(&job_id);

    let cache = app.state::<ObjectCacheManager>();
    if let Some(mut meta) = cache.get_bucket_index_meta(&connection_id, &bucket) {
        meta.status = "running".to_string();
        let _ = cache.upsert_bucket_index_meta(&meta);
    }

    Ok(())
}

#[tauri::command]
pub async fn cancel_bucket_index(
    app: AppHandle,
    connection_id: String,
    bucket: String,
) -> Result<(), PakerError> {
    let job_id = bucket_index_job_id(&connection_id, &bucket);
    app.state::<TransferManager>().cancel(&job_id);
    Ok(())
}

#[tauri::command]
pub async fn search_bucket_index(
    app: AppHandle,
    connection_id: String,
    bucket: String,
    query: String,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<IndexedObject>, PakerError> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }

    let cache = app.state::<ObjectCacheManager>();
    cache
        .search_bucket_index(
            &connection_id,
            &bucket,
            query,
            limit.unwrap_or(500),
            offset.unwrap_or(0),
        )
        .map_err(into_ipc_error)
}

#[tauri::command]
pub async fn export_bucket_index_csv(
    app: AppHandle,
    connection_id: String,
    bucket: String,
    save_path: Option<String>,
) -> Result<String, PakerError> {
    let cache = app.state::<ObjectCacheManager>();
    let csv = cache
        .export_bucket_index_csv(&connection_id, &bucket)
        .map_err(into_ipc_error)?;

    let scope = app.state::<LocalFsScope>();
    let dest: PathBuf = match save_path.filter(|p| !p.is_empty()) {
        Some(path) => {
            let dest = PathBuf::from(path);
            scope.validate_export_path(&dest)?;
            dest
        }
        None => {
            let picked = rfd::FileDialog::new()
                .set_title("Save bucket index CSV")
                .set_file_name(format!("{bucket}-index.csv"))
                .add_filter("CSV", &["csv"])
                .save_file()
                .ok_or_else(|| {
                    PakerError::InvalidInput("Export cancelled: no file selected".to_string())
                })?;
            if let Some(parent) = picked.parent() {
                scope.register_picked_folder(parent);
            }
            picked
        }
    };

    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|_| PakerError::PathNotAllowed)?;
    }

    crate::storage::paths::write_private_file(&dest, csv.as_bytes()).map_err(into_ipc_error)?;
    Ok(dest.to_string_lossy().into_owned())
}
