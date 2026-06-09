use crate::error::PakerError;
use crate::storage::paths;
use crate::storage::ui_state;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fs;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::AppHandle;

const GITHUB_LATEST_RELEASE_URL: &str =
    "https://api.github.com/repos/kkopanidis/paker/releases/latest";
const CACHE_TTL: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub update_available: bool,
    pub release_url: String,
    pub release_name: String,
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    name: String,
    html_url: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateCheckCache {
    checked_at_secs: u64,
    info: UpdateInfo,
}

fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

fn user_agent() -> String {
    format!("Paker/{}", current_version())
}

fn no_update_info() -> UpdateInfo {
    let version = current_version().to_string();
    UpdateInfo {
        current_version: version.clone(),
        latest_version: version,
        update_available: false,
        release_url: String::new(),
        release_name: String::new(),
    }
}

fn strip_version_prefix(tag: &str) -> &str {
    tag.strip_prefix('v')
        .or_else(|| tag.strip_prefix('V'))
        .unwrap_or(tag)
}

fn version_parts(version: &str) -> Vec<u64> {
    version
        .split(['.', '-', '+'])
        .filter_map(|part| part.parse::<u64>().ok())
        .collect()
}

fn compare_versions(latest: &str, current: &str) -> Ordering {
    let latest_parts = version_parts(latest);
    let current_parts = version_parts(current);
    let max_len = latest_parts.len().max(current_parts.len());

    for index in 0..max_len {
        let left = latest_parts.get(index).copied().unwrap_or(0);
        let right = current_parts.get(index).copied().unwrap_or(0);
        match left.cmp(&right) {
            Ordering::Equal => continue,
            other => return other,
        }
    }

    Ordering::Equal
}

fn is_newer_version(latest: &str, current: &str) -> bool {
    compare_versions(latest, current) == Ordering::Greater
}

fn parse_github_release(json: &str) -> Result<GitHubRelease> {
    serde_json::from_str(json).context("failed to parse GitHub release response")
}

fn update_info_from_release(release: &GitHubRelease) -> UpdateInfo {
    let current = current_version().to_string();
    let latest = strip_version_prefix(&release.tag_name).to_string();
    UpdateInfo {
        current_version: current.clone(),
        latest_version: latest.clone(),
        update_available: is_newer_version(&latest, &current),
        release_url: release.html_url.clone(),
        release_name: release.name.clone(),
    }
}

fn read_cache(app: &AppHandle) -> Result<Option<UpdateInfo>> {
    let path = paths::update_check_cache_path(app)?;
    if !path.exists() {
        return Ok(None);
    }

    let contents =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    if contents.trim().is_empty() {
        return Ok(None);
    }

    let cache: UpdateCheckCache =
        serde_json::from_str(&contents).context("failed to parse update check cache")?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system time before UNIX epoch")?
        .as_secs();

    if now.saturating_sub(cache.checked_at_secs) >= CACHE_TTL.as_secs() {
        return Ok(None);
    }

    Ok(Some(cache.info))
}

fn write_cache(app: &AppHandle, info: &UpdateInfo) -> Result<()> {
    let path = paths::update_check_cache_path(app)?;
    paths::ensure_parent(&path)?;
    let checked_at_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system time before UNIX epoch")?
        .as_secs();
    let cache = UpdateCheckCache {
        checked_at_secs,
        info: info.clone(),
    };
    let contents =
        serde_json::to_string_pretty(&cache).context("failed to serialize update check cache")?;
    paths::write_private_file(&path, contents.as_bytes())
}

fn fetch_latest_release() -> Result<GitHubRelease> {
    let client = reqwest::blocking::Client::builder()
        .user_agent(user_agent())
        .build()
        .context("failed to build HTTP client")?;

    let response = client
        .get(GITHUB_LATEST_RELEASE_URL)
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .send()
        .context("failed to request GitHub releases API")?;

    if !response.status().is_success() {
        anyhow::bail!("GitHub releases API returned {}", response.status());
    }

    let body = response
        .text()
        .context("failed to read GitHub release response body")?;
    parse_github_release(&body)
}

fn check_for_update_inner(app: &AppHandle) -> UpdateInfo {
    if !ui_state::get_ui_preferences(app).check_for_updates {
        return no_update_info();
    }

    if let Ok(Some(cached)) = read_cache(app) {
        return cached;
    }

    let release = match fetch_latest_release() {
        Ok(release) => release,
        Err(_) => return no_update_info(),
    };

    let info = update_info_from_release(&release);

    let _ = write_cache(app, &info);
    info
}

#[tauri::command]
pub async fn check_for_update(app: AppHandle) -> Result<UpdateInfo, PakerError> {
    tauri::async_runtime::spawn_blocking(move || check_for_update_inner(&app))
        .await
        .map_err(|_| PakerError::Internal)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RELEASE_JSON: &str = r#"{
        "tag_name": "v1.2.3",
        "name": "Paker 1.2.3",
        "html_url": "https://github.com/kkopanidis/paker/releases/tag/v1.2.3"
    }"#;

    #[test]
    fn parse_github_release_reads_expected_fields() {
        let release = parse_github_release(SAMPLE_RELEASE_JSON).expect("parse");
        assert_eq!(release.tag_name, "v1.2.3");
        assert_eq!(release.name, "Paker 1.2.3");
        assert_eq!(
            release.html_url,
            "https://github.com/kkopanidis/paker/releases/tag/v1.2.3"
        );
    }

    #[test]
    fn strip_version_prefix_removes_v_prefix() {
        assert_eq!(strip_version_prefix("v1.0.0"), "1.0.0");
        assert_eq!(strip_version_prefix("V2.0.0"), "2.0.0");
        assert_eq!(strip_version_prefix("1.0.0"), "1.0.0");
    }

    #[test]
    fn compare_versions_orders_semver_like_strings() {
        assert_eq!(compare_versions("1.2.0", "1.1.9"), Ordering::Greater);
        assert_eq!(compare_versions("1.0.0", "1.0.0"), Ordering::Equal);
        assert_eq!(compare_versions("0.9.9", "1.0.0"), Ordering::Less);
        assert_eq!(compare_versions("1.0.0-beta", "1.0.0"), Ordering::Equal);
        assert_eq!(compare_versions("1.0.0", "1.0.0.1"), Ordering::Less);
    }

    #[test]
    fn is_newer_version_detects_update() {
        assert!(is_newer_version("9.9.9", current_version()));
        assert!(!is_newer_version(current_version(), current_version()));
        assert!(!is_newer_version("0.0.1", current_version()));
    }

    #[test]
    fn update_info_from_release_marks_newer_tag_as_available() {
        let release = parse_github_release(SAMPLE_RELEASE_JSON).expect("parse");
        let info = update_info_from_release(&release);
        assert_eq!(info.current_version, current_version());
        assert_eq!(info.latest_version, "1.2.3");
        assert_eq!(info.release_name, "Paker 1.2.3");
        assert!(info.release_url.contains("github.com"));
        assert_eq!(
            info.update_available,
            is_newer_version("1.2.3", current_version())
        );
    }
}
