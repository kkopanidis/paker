use super::paths;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use tauri::AppHandle;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionProfile {
    pub id: String,
    pub name: String,
    pub endpoint: Option<String>,
    pub region: String,
    pub access_key_id: String,
    pub force_path_style: bool,
    pub default_bucket: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveConnectionInput {
    pub id: Option<String>,
    pub name: String,
    pub endpoint: Option<String>,
    pub region: String,
    pub access_key_id: String,
    pub secret_access_key: Option<String>,
    pub session_token: Option<String>,
    pub force_path_style: bool,
    pub default_bucket: Option<String>,
}

fn read_all(app: &AppHandle) -> Result<Vec<ConnectionProfile>> {
    let path = paths::connections_path(app)?;
    if !path.exists() {
        return Ok(Vec::new());
    }

    let contents =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    if contents.trim().is_empty() {
        return Ok(Vec::new());
    }

    serde_json::from_str(&contents).context("failed to parse connections.json")
}

fn write_all(app: &AppHandle, profiles: &[ConnectionProfile]) -> Result<()> {
    let path = paths::connections_path(app)?;
    paths::ensure_parent(&path)?;
    let contents =
        serde_json::to_string_pretty(profiles).context("failed to serialize connections")?;
    paths::write_private_file(&path, contents.as_bytes())
}

pub fn list_connections(app: &AppHandle) -> Result<Vec<ConnectionProfile>> {
    read_all(app)
}

pub fn get_connection(app: &AppHandle, id: &str) -> Result<Option<ConnectionProfile>> {
    Ok(read_all(app)?.into_iter().find(|profile| profile.id == id))
}

pub fn save_connection(app: &AppHandle, input: SaveConnectionInput) -> Result<ConnectionProfile> {
    let mut profiles = read_all(app)?;
    let id = input
        .id
        .clone()
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    let profile = ConnectionProfile {
        id: id.clone(),
        name: input.name,
        endpoint: input.endpoint,
        region: input.region,
        access_key_id: input.access_key_id,
        force_path_style: input.force_path_style,
        default_bucket: input.default_bucket,
    };

    if let Some(index) = profiles.iter().position(|p| p.id == id) {
        profiles[index] = profile.clone();
    } else {
        profiles.push(profile.clone());
    }

    write_all(app, &profiles)?;
    Ok(profile)
}

pub fn delete_connection(app: &AppHandle, id: &str) -> Result<bool> {
    let mut profiles = read_all(app)?;
    let before = profiles.len();
    profiles.retain(|profile| profile.id != id);
    if profiles.len() == before {
        return Ok(false);
    }
    write_all(app, &profiles)?;
    Ok(true)
}
