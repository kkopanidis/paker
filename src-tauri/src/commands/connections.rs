use crate::s3::build_client;
use crate::storage::{self, delete_secret, set_secrets, ConnectionProfile, SaveConnectionInput};
use crate::s3::operations::{list_buckets as s3_list_buckets, verify_bucket_access};
use tauri::AppHandle;

fn map_err(err: impl ToString) -> String {
    err.to_string()
}

#[tauri::command]
pub async fn list_connections(app: AppHandle) -> Result<Vec<ConnectionProfile>, String> {
    storage::list_connections(&app).map_err(map_err)
}

#[tauri::command]
pub async fn get_connection(
    app: AppHandle,
    id: String,
) -> Result<Option<ConnectionProfile>, String> {
    storage::get_connection(&app, &id).map_err(map_err)
}

#[tauri::command]
pub async fn save_connection(
    app: AppHandle,
    input: SaveConnectionInput,
) -> Result<ConnectionProfile, String> {
    let secret = input.secret_access_key.clone();
    let session_token = input.session_token.clone();
    let profile = storage::save_connection(&app, input).map_err(map_err)?;

    if let Some(secret) = secret {
        if !secret.is_empty() {
            let token = session_token
                .as_deref()
                .filter(|t| !t.is_empty());
            set_secrets(&app, &profile.id, &secret, token).map_err(map_err)?;
        }
    }

    Ok(profile)
}

#[tauri::command]
pub async fn delete_connection(app: AppHandle, id: String) -> Result<bool, String> {
    let deleted = storage::delete_connection(&app, &id).map_err(map_err)?;
    if deleted {
        delete_secret(&app, &id).map_err(map_err)?;
    }
    Ok(deleted)
}

#[tauri::command]
pub async fn test_connection(app: AppHandle, id: String) -> Result<(), String> {
    let profile = storage::get_connection(&app, &id)
        .map_err(map_err)?
        .ok_or_else(|| format!("connection not found: {id}"))?;

    let client = build_client(&app, &profile).await.map_err(map_err)?;

    if let Some(bucket) = profile
        .default_bucket
        .as_ref()
        .map(|b| b.trim())
        .filter(|b| !b.is_empty())
    {
        verify_bucket_access(&client, bucket)
            .await
            .map_err(map_err)?;
        return Ok(());
    }

    s3_list_buckets(&client).await.map_err(map_err)?;
    Ok(())
}
