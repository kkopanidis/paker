use crate::storage::ui_state as state;
use serde::Serialize;
use tauri::AppHandle;

fn map_err(err: impl ToString) -> String {
    err.to_string()
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferSettings {
    pub max_concurrent_transfers: u32,
}

#[tauri::command]
pub async fn get_last_local_dir(
    app: AppHandle,
    connection_id: String,
) -> Option<String> {
    state::get_last_local_dir(&app, &connection_id)
}

#[tauri::command]
pub async fn set_last_local_dir(
    app: AppHandle,
    connection_id: String,
    path: String,
) -> Result<(), String> {
    state::set_last_local_dir(&app, &connection_id, &path).map_err(map_err)
}

#[tauri::command]
pub async fn get_transfer_settings(app: AppHandle) -> TransferSettings {
    TransferSettings {
        max_concurrent_transfers: state::get_max_concurrent_transfers(&app),
    }
}
