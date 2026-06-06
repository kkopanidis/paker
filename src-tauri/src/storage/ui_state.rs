use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use tauri::AppHandle;

use super::paths;

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UiState {
    #[serde(default)]
    last_local_dir: HashMap<String, String>,
    #[serde(default = "default_max_concurrent")]
    max_concurrent_transfers: u32,
}

fn default_max_concurrent() -> u32 {
    3
}

fn read_state(app: &AppHandle) -> Result<UiState> {
    let path = paths::ui_state_path(app)?;
    if !path.exists() {
        return Ok(UiState::default());
    }
    let contents = fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    if contents.trim().is_empty() {
        return Ok(UiState::default());
    }
    serde_json::from_str(&contents).context("failed to parse ui_state.json")
}

fn write_state(app: &AppHandle, state: &UiState) -> Result<()> {
    let path = paths::ui_state_path(app)?;
    paths::ensure_parent(&path)?;
    let contents = serde_json::to_string_pretty(state).context("failed to serialize ui state")?;
    fs::write(&path, contents).with_context(|| format!("failed to write {}", path.display()))
}

pub fn get_last_local_dir(app: &AppHandle, connection_id: &str) -> Option<String> {
    read_state(app)
        .ok()?
        .last_local_dir
        .get(connection_id)
        .cloned()
}

pub fn set_last_local_dir(app: &AppHandle, connection_id: &str, path: &str) -> Result<()> {
    let mut state = read_state(app)?;
    state
        .last_local_dir
        .insert(connection_id.to_owned(), path.to_owned());
    write_state(app, &state)
}

pub fn get_max_concurrent_transfers(app: &AppHandle) -> u32 {
    read_state(app)
        .map(|s| s.max_concurrent_transfers)
        .unwrap_or(3)
}

#[allow(dead_code)]
pub fn set_max_concurrent_transfers(app: &AppHandle, value: u32) -> Result<()> {
    let mut state = read_state(app)?;
    state.max_concurrent_transfers = value;
    write_state(app, &state)
}
