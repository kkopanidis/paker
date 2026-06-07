use crate::error::{into_ipc_error, validate_endpoint_url, PakerError};
use crate::s3::build_client;
use crate::s3::operations::{list_buckets as s3_list_buckets, verify_bucket_access};
use crate::storage::{self, delete_secret, set_secrets, ConnectionProfile, SaveConnectionInput};
use tauri::AppHandle;

#[tauri::command]
pub async fn list_connections(app: AppHandle) -> Result<Vec<ConnectionProfile>, PakerError> {
    storage::list_connections(&app).map_err(into_ipc_error)
}

#[tauri::command]
pub async fn get_connection(
    app: AppHandle,
    id: String,
) -> Result<Option<ConnectionProfile>, PakerError> {
    storage::get_connection(&app, &id).map_err(into_ipc_error)
}

#[tauri::command]
pub async fn save_connection(
    app: AppHandle,
    input: SaveConnectionInput,
) -> Result<ConnectionProfile, PakerError> {
    if let Some(endpoint) = input.endpoint.as_deref() {
        validate_endpoint_url(endpoint)?;
    }

    let secret = input.secret_access_key.clone();
    let session_token = input.session_token.clone();
    let profile = storage::save_connection(&app, input).map_err(into_ipc_error)?;

    if let Some(secret) = secret {
        if !secret.is_empty() {
            let token = session_token.as_deref().filter(|t| !t.is_empty());
            set_secrets(&app, &profile.id, &secret, token).map_err(into_ipc_error)?;
        }
    }

    Ok(profile)
}

#[tauri::command]
pub async fn delete_connection(app: AppHandle, id: String) -> Result<bool, PakerError> {
    let deleted = storage::delete_connection(&app, &id).map_err(into_ipc_error)?;
    if deleted {
        delete_secret(&app, &id).map_err(into_ipc_error)?;
    }
    Ok(deleted)
}

#[tauri::command]
pub async fn test_connection(app: AppHandle, id: String) -> Result<(), PakerError> {
    let profile = storage::get_connection(&app, &id)
        .map_err(into_ipc_error)?
        .ok_or(PakerError::ConnectionNotFound)?;

    let client = build_client(&app, &profile).await?;

    if let Some(bucket) = profile
        .default_bucket
        .as_ref()
        .map(|b| b.trim())
        .filter(|b| !b.is_empty())
    {
        verify_bucket_access(&client, bucket).await?;
        return Ok(());
    }

    s3_list_buckets(&client).await?;
    Ok(())
}
