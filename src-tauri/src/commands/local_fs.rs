use crate::path_safety::is_path_under_root;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::UNIX_EPOCH;
use tauri::{AppHandle, Manager};

fn map_err(err: impl ToString) -> String {
    err.to_string()
}

fn home_dir() -> Result<PathBuf, String> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .map_err(|_| "could not determine home directory".to_string())
}

/// Tracks directories the UI may browse via `list_local_dir`.
pub struct LocalFsScope {
    home_dir: PathBuf,
    picked_roots: Mutex<Vec<PathBuf>>,
}

impl LocalFsScope {
    pub fn new() -> Result<Self, String> {
        let home = home_dir()?;
        let home_dir = home.canonicalize().unwrap_or(home);
        Ok(Self {
            home_dir,
            picked_roots: Mutex::new(Vec::new()),
        })
    }

    pub fn home_dir(&self) -> &Path {
        &self.home_dir
    }

    pub fn register_picked_folder(&self, path: &Path) {
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let mut roots = self.picked_roots.lock().expect("picked_roots lock");
        if !roots.iter().any(|root| root == &canonical) {
            roots.push(canonical);
        }
    }

    pub fn validate_access(&self, path: &Path) -> Result<PathBuf, String> {
        if !path.exists() {
            return Err(format!("path does not exist: {}", path.display()));
        }

        let canonical = path
            .canonicalize()
            .map_err(|e| format!("invalid path {}: {e}", path.display()))?;

        if is_path_under_root(&canonical, &self.home_dir) {
            return Ok(canonical);
        }

        let roots = self.picked_roots.lock().expect("picked_roots lock");
        if roots
            .iter()
            .any(|root| is_path_under_root(&canonical, root))
        {
            return Ok(canonical);
        }

        Err(format!(
            "path is outside allowed directories (home and folders opened via picker): {}",
            path.display()
        ))
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: Option<String>,
}

#[tauri::command]
pub async fn list_local_dir(app: AppHandle, path: String) -> Result<Vec<LocalEntry>, String> {
    let scope = app.state::<LocalFsScope>();
    let dir = scope.validate_access(Path::new(&path))?;
    let read = fs::read_dir(&dir).map_err(map_err)?;

    let mut entries: Vec<LocalEntry> = read
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let meta = entry.metadata().ok()?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = entry.path().to_string_lossy().into_owned();
            let is_dir = meta.is_dir();
            let size = if is_dir { 0 } else { meta.len() };
            let modified = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| format_unix_timestamp(d.as_secs()));

            Some(LocalEntry {
                name,
                path,
                is_dir,
                size,
                modified,
            })
        })
        .collect();

    entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });

    Ok(entries)
}

#[tauri::command]
pub async fn get_home_dir(app: AppHandle) -> Result<String, String> {
    Ok(app
        .state::<LocalFsScope>()
        .home_dir()
        .to_string_lossy()
        .into_owned())
}

#[tauri::command]
pub async fn pick_local_folder(app: AppHandle) -> Result<Option<String>, String> {
    let folder = rfd::FileDialog::new()
        .set_title("Select folder")
        .pick_folder();

    if let Some(ref picked) = folder {
        app.state::<LocalFsScope>()
            .register_picked_folder(picked.as_path());
    }

    Ok(folder.map(|p| p.to_string_lossy().into_owned()))
}

#[tauri::command]
pub async fn get_parent_path(path: String) -> Result<Option<String>, String> {
    let parent = Path::new(&path)
        .parent()
        .map(|p| p.to_string_lossy().into_owned());
    Ok(parent)
}

fn format_unix_timestamp(secs: u64) -> String {
    // Produce a simple ISO-8601-ish timestamp without pulling in chrono.
    // Callers can use JS Date.parse() on the result.
    let secs_i64 = secs as i64;

    // Days since Unix epoch
    let days = secs_i64 / 86400;
    let rem = secs_i64 % 86400;
    let hour = rem / 3600;
    let min = (rem % 3600) / 60;
    let sec = rem % 60;

    // Civil date from day count (proleptic Gregorian, Z era only)
    let (year, month, day) = days_to_ymd(days);

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hour, min, sec
    )
}

fn days_to_ymd(z: i64) -> (i64, i64, i64) {
    // Algorithm from https://howardhinnant.github.io/date_algorithms.html
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_paths_under_home() {
        let scope = LocalFsScope::new().expect("scope");
        let allowed = scope
            .validate_access(scope.home_dir())
            .expect("home should be allowed");
        assert_eq!(allowed, scope.home_dir().canonicalize().unwrap_or_else(|_| scope.home_dir().to_path_buf()));
    }

    #[test]
    fn allows_paths_under_picked_folder_outside_home() {
        let scope = LocalFsScope::new().expect("scope");
        let picked = std::env::temp_dir();
        scope.register_picked_folder(&picked);
        let allowed = scope.validate_access(&picked).expect("picked root allowed");
        assert_eq!(
            allowed,
            picked.canonicalize().unwrap_or_else(|_| picked.clone())
        );
    }

    #[test]
    fn rejects_paths_outside_allowed_roots() {
        let scope = LocalFsScope::new().expect("scope");
        let blocked = if cfg!(windows) {
            PathBuf::from(r"C:\Windows")
        } else {
            PathBuf::from("/etc")
        };
        if !blocked.exists() {
            return;
        }
        let err = scope
            .validate_access(&blocked)
            .expect_err("system path should be rejected");
        assert!(err.contains("outside allowed directories"));
    }
}
