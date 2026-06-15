use super::paths;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
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
    read_all_from_path(&path)
}

fn read_all_from_path(path: &std::path::PathBuf) -> Result<Vec<ConnectionProfile>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let contents =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    if contents.trim().is_empty() {
        return Ok(Vec::new());
    }

    serde_json::from_str(&contents).context("failed to parse connections.json")
}

/// Standalone variant: read all connection profiles from an explicit data directory.
/// Used by `paker-mcp` which has no Tauri runtime.
pub fn list_connections_from(base: &Path) -> Result<Vec<ConnectionProfile>> {
    read_all_from_path(&paths::connections_path_in(base))
}

/// Standalone variant: look up a single connection profile by ID.
pub fn get_connection_from(base: &Path, id: &str) -> Result<Option<ConnectionProfile>> {
    Ok(list_connections_from(base)?
        .into_iter()
        .find(|p| p.id == id))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_profile() -> ConnectionProfile {
        ConnectionProfile {
            id: "conn-1".to_string(),
            name: "MinIO".to_string(),
            endpoint: Some("https://minio.example.com".to_string()),
            region: "us-east-1".to_string(),
            access_key_id: "AKIAEXAMPLE".to_string(),
            force_path_style: true,
            default_bucket: Some("media".to_string()),
        }
    }

    #[test]
    fn connection_profile_round_trip_serialization() {
        let profile = sample_profile();
        let json = serde_json::to_string(&profile).expect("serialize");
        assert!(json.contains("\"accessKeyId\""));
        assert!(json.contains("\"forcePathStyle\""));

        let parsed: ConnectionProfile = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.id, profile.id);
        assert_eq!(parsed.name, profile.name);
        assert_eq!(parsed.endpoint, profile.endpoint);
        assert_eq!(parsed.region, profile.region);
        assert_eq!(parsed.access_key_id, profile.access_key_id);
        assert_eq!(parsed.force_path_style, profile.force_path_style);
        assert_eq!(parsed.default_bucket, profile.default_bucket);
    }

    #[test]
    fn connection_profile_defaults_optional_fields_to_none() {
        let json = r#"{
            "id": "conn-aws",
            "name": "AWS",
            "region": "eu-west-1",
            "accessKeyId": "KEY",
            "forcePathStyle": false
        }"#;
        let profile: ConnectionProfile = serde_json::from_str(json).expect("deserialize");
        assert!(profile.endpoint.is_none());
        assert!(profile.default_bucket.is_none());
        assert!(!profile.force_path_style);
    }

    #[test]
    fn connection_profile_endpoint_handling() {
        let with_endpoint = r#"{
            "id": "1",
            "name": "Local",
            "endpoint": "http://localhost:9000",
            "region": "us-east-1",
            "accessKeyId": "minio",
            "forcePathStyle": true
        }"#;
        let profile: ConnectionProfile = serde_json::from_str(with_endpoint).expect("deserialize");
        assert_eq!(profile.endpoint.as_deref(), Some("http://localhost:9000"));

        let without_endpoint = r#"{
            "id": "2",
            "name": "AWS",
            "region": "us-east-1",
            "accessKeyId": "AKIA",
            "forcePathStyle": false
        }"#;
        let profile: ConnectionProfile =
            serde_json::from_str(without_endpoint).expect("deserialize");
        assert!(profile.endpoint.is_none());
    }

    #[test]
    fn save_connection_input_deserializes_camel_case_fields() {
        let json = r#"{
            "name": "Staging",
            "endpoint": "https://s3.example.com",
            "region": "auto",
            "accessKeyId": "key",
            "secretAccessKey": "secret",
            "sessionToken": "token",
            "forcePathStyle": true,
            "defaultBucket": "uploads"
        }"#;
        let input: SaveConnectionInput = serde_json::from_str(json).expect("deserialize");
        assert_eq!(input.name, "Staging");
        assert_eq!(input.endpoint.as_deref(), Some("https://s3.example.com"));
        assert_eq!(input.secret_access_key.as_deref(), Some("secret"));
        assert_eq!(input.session_token.as_deref(), Some("token"));
        assert!(input.force_path_style);
        assert_eq!(input.default_bucket.as_deref(), Some("uploads"));
    }
}
