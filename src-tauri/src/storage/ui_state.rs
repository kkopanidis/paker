use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use tauri::AppHandle;

use super::paths;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionNav {
    pub bucket: Option<String>,
    pub prefix: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrefixBookmark {
    pub id: String,
    pub label: String,
    pub bucket: String,
    pub prefix: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UiPreferences {
    #[serde(default)]
    pub local_panel_open: bool,
    #[serde(default = "default_details_open")]
    pub details_pane_open: bool,
    #[serde(default = "default_true")]
    pub connections_collapsed: bool,
    #[serde(default = "default_true")]
    pub buckets_collapsed: bool,
}

fn default_details_open() -> bool {
    true
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FullUiState {
    pub last_local_dir: HashMap<String, String>,
    pub max_concurrent_transfers: u32,
    pub last_nav: HashMap<String, ConnectionNav>,
    pub bookmarks: HashMap<String, Vec<PrefixBookmark>>,
    pub preferences: UiPreferences,
    pub panel_layout_three: Option<HashMap<String, f64>>,
    pub panel_layout_four: Option<HashMap<String, f64>>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UiState {
    #[serde(default)]
    last_local_dir: HashMap<String, String>,
    #[serde(default = "default_max_concurrent")]
    max_concurrent_transfers: u32,
    #[serde(default)]
    last_nav: HashMap<String, ConnectionNav>,
    #[serde(default)]
    bookmarks: HashMap<String, Vec<PrefixBookmark>>,
    #[serde(default)]
    preferences: UiPreferences,
    #[serde(default)]
    panel_layout_three: Option<HashMap<String, f64>>,
    #[serde(default)]
    panel_layout_four: Option<HashMap<String, f64>>,
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

impl From<UiState> for FullUiState {
    fn from(state: UiState) -> Self {
        Self {
            last_local_dir: state.last_local_dir,
            max_concurrent_transfers: state.max_concurrent_transfers,
            last_nav: state.last_nav,
            bookmarks: state.bookmarks,
            preferences: state.preferences,
            panel_layout_three: state.panel_layout_three,
            panel_layout_four: state.panel_layout_four,
        }
    }
}

pub fn get_full_ui_state(app: &AppHandle) -> Result<FullUiState> {
    read_state(app).map(FullUiState::from)
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

pub fn get_connection_nav(app: &AppHandle, connection_id: &str) -> Option<ConnectionNav> {
    read_state(app)
        .ok()?
        .last_nav
        .get(connection_id)
        .cloned()
}

pub fn set_connection_nav(
    app: &AppHandle,
    connection_id: &str,
    nav: ConnectionNav,
) -> Result<()> {
    let mut state = read_state(app)?;
    state
        .last_nav
        .insert(connection_id.to_owned(), nav);
    write_state(app, &state)
}

pub fn get_bookmarks(app: &AppHandle, connection_id: &str) -> Vec<PrefixBookmark> {
    read_state(app)
        .ok()
        .and_then(|s| s.bookmarks.get(connection_id).cloned())
        .unwrap_or_default()
}

pub fn add_bookmark(
    app: &AppHandle,
    connection_id: &str,
    bookmark: PrefixBookmark,
) -> Result<()> {
    let mut state = read_state(app)?;
    state
        .bookmarks
        .entry(connection_id.to_owned())
        .or_default()
        .push(bookmark);
    write_state(app, &state)
}

pub fn remove_bookmark(
    app: &AppHandle,
    connection_id: &str,
    bookmark_id: &str,
) -> Result<()> {
    let mut state = read_state(app)?;
    if let Some(bookmarks) = state.bookmarks.get_mut(connection_id) {
        bookmarks.retain(|b| b.id != bookmark_id);
        if bookmarks.is_empty() {
            state.bookmarks.remove(connection_id);
        }
    }
    write_state(app, &state)
}

pub fn get_ui_preferences(app: &AppHandle) -> UiPreferences {
    read_state(app)
        .map(|s| s.preferences)
        .unwrap_or_default()
}

pub fn set_ui_preferences(app: &AppHandle, prefs: UiPreferences) -> Result<()> {
    let mut state = read_state(app)?;
    state.preferences = prefs;
    write_state(app, &state)
}

pub fn get_panel_layout(
    app: &AppHandle,
    mode: &str,
) -> Result<Option<HashMap<String, f64>>> {
    let state = read_state(app)?;
    Ok(match mode {
        "three" => state.panel_layout_three,
        "four" => state.panel_layout_four,
        other => anyhow::bail!("invalid panel layout mode: {other}"),
    })
}

pub fn set_panel_layout(
    app: &AppHandle,
    mode: &str,
    layout: HashMap<String, f64>,
) -> Result<()> {
    let mut state = read_state(app)?;
    match mode {
        "three" => state.panel_layout_three = Some(layout),
        "four" => state.panel_layout_four = Some(layout),
        other => anyhow::bail!("invalid panel layout mode: {other}"),
    }
    write_state(app, &state)
}
