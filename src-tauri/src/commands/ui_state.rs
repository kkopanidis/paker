use crate::storage::ui_state as state;
use crate::storage::ui_state::{
    ConnectionNav, FullUiState, PrefixBookmark, UiPreferences,
};
use serde::Serialize;
use std::collections::HashMap;
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

#[tauri::command]
pub async fn get_full_ui_state(app: AppHandle) -> Result<FullUiState, String> {
    state::get_full_ui_state(&app).map_err(map_err)
}

#[tauri::command]
pub async fn get_connection_nav(
    app: AppHandle,
    connection_id: String,
) -> Option<ConnectionNav> {
    state::get_connection_nav(&app, &connection_id)
}

#[tauri::command]
pub async fn set_connection_nav(
    app: AppHandle,
    connection_id: String,
    nav: ConnectionNav,
) -> Result<(), String> {
    state::set_connection_nav(&app, &connection_id, nav).map_err(map_err)
}

#[tauri::command]
pub async fn get_bookmarks(
    app: AppHandle,
    connection_id: String,
) -> Vec<PrefixBookmark> {
    state::get_bookmarks(&app, &connection_id)
}

#[tauri::command]
pub async fn add_bookmark(
    app: AppHandle,
    connection_id: String,
    bookmark: PrefixBookmark,
) -> Result<(), String> {
    state::add_bookmark(&app, &connection_id, bookmark).map_err(map_err)
}

#[tauri::command]
pub async fn remove_bookmark(
    app: AppHandle,
    connection_id: String,
    bookmark_id: String,
) -> Result<(), String> {
    state::remove_bookmark(&app, &connection_id, &bookmark_id).map_err(map_err)
}

#[tauri::command]
pub async fn get_ui_preferences(app: AppHandle) -> UiPreferences {
    state::get_ui_preferences(&app)
}

#[tauri::command]
pub async fn set_ui_preferences(
    app: AppHandle,
    preferences: UiPreferences,
) -> Result<(), String> {
    state::set_ui_preferences(&app, preferences).map_err(map_err)
}

#[tauri::command]
pub async fn get_panel_layout(
    app: AppHandle,
    mode: String,
) -> Result<Option<HashMap<String, f64>>, String> {
    state::get_panel_layout(&app, &mode).map_err(map_err)
}

#[tauri::command]
pub async fn set_panel_layout(
    app: AppHandle,
    mode: String,
    layout: HashMap<String, f64>,
) -> Result<(), String> {
    state::set_panel_layout(&app, &mode, layout).map_err(map_err)
}
