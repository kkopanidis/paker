use anyhow::{Context, Result};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

const PORTABLE_MARKER: &str = "portable.txt";

pub fn is_portable_mode() -> bool {
    if env::var("PAKER_PORTABLE")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
    {
        return true;
    }

    portable_marker_path()
        .map(|p| p.is_file())
        .unwrap_or(false)
}

fn portable_marker_path() -> Option<PathBuf> {
    let exe = env::current_exe().ok()?;
    let exe_dir = exe.parent()?;
    Some(exe_dir.join(PORTABLE_MARKER))
}

fn portable_data_dir() -> Result<PathBuf> {
    let exe = env::current_exe().context("failed to resolve current executable")?;
    let exe_dir = exe
        .parent()
        .context("executable has no parent directory")?;
    Ok(exe_dir.join("data"))
}

pub fn data_dir(app: &AppHandle) -> Result<PathBuf> {
    let dir = if is_portable_mode() {
        portable_data_dir()?
    } else {
        app.path()
            .app_data_dir()
            .context("failed to resolve app data directory")?
    };

    fs::create_dir_all(&dir)
        .with_context(|| format!("failed to create data directory {}", dir.display()))?;

    Ok(dir)
}

pub fn connections_path(app: &AppHandle) -> Result<PathBuf> {
    Ok(data_dir(app)?.join("connections.json"))
}

pub fn secrets_path(app: &AppHandle) -> Result<PathBuf> {
    Ok(data_dir(app)?.join("secrets.enc"))
}

pub fn ui_state_path(app: &AppHandle) -> Result<PathBuf> {
    Ok(data_dir(app)?.join("ui_state.json"))
}

pub fn ensure_parent(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }
    Ok(())
}
