use crate::error::PakerError;
use crate::path_safety::is_path_under_root;
use crate::storage::paths::preview_cache_dir;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::UNIX_EPOCH;
use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;

fn home_dir() -> Result<PathBuf, PakerError> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .map_err(|_| PakerError::Internal)
}

/// Tracks directories and files the UI may access via local filesystem commands.
pub struct LocalFsScope {
    home_dir: PathBuf,
    picked_roots: Mutex<Vec<PathBuf>>,
    /// Files explicitly allowed for upload (picker / drag-drop session).
    allowed_files: Mutex<Vec<PathBuf>>,
}

impl LocalFsScope {
    pub fn new() -> Result<Self, PakerError> {
        let home = home_dir()?;
        let home_dir = home.canonicalize().unwrap_or(home);
        Ok(Self {
            home_dir,
            picked_roots: Mutex::new(Vec::new()),
            allowed_files: Mutex::new(Vec::new()),
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

    pub fn register_file_paths(&self, paths: &[PathBuf]) {
        let mut files = self.allowed_files.lock().expect("allowed_files lock");
        for path in paths {
            if let Ok(canonical) = path.canonicalize() {
                if canonical.is_file() && !files.iter().any(|f| f == &canonical) {
                    files.push(canonical);
                }
            }
        }
    }

    fn is_under_scope(&self, canonical: &Path) -> bool {
        if is_path_under_root(canonical, &self.home_dir) {
            return true;
        }
        let roots = self.picked_roots.lock().expect("picked_roots lock");
        roots.iter().any(|root| is_path_under_root(canonical, root))
    }

    pub fn validate_access(&self, path: &Path) -> Result<PathBuf, PakerError> {
        if !path.exists() {
            return Err(PakerError::PathNotAllowed);
        }

        let canonical = path
            .canonicalize()
            .map_err(|_| PakerError::PathNotAllowed)?;

        if self.is_under_scope(&canonical) {
            return Ok(canonical);
        }

        Err(PakerError::PathNotAllowed)
    }

    pub fn validate_dir_access(&self, path: &Path) -> Result<PathBuf, PakerError> {
        let canonical = self.validate_access(path)?;
        let meta = fs::metadata(&canonical).map_err(|_| PakerError::PathNotAllowed)?;
        if !meta.is_dir() {
            return Err(PakerError::InvalidInput(
                "Path is not a directory".to_string(),
            ));
        }
        Ok(canonical)
    }

    pub fn validate_file_access(&self, path: &Path) -> Result<PathBuf, PakerError> {
        if !path.exists() {
            return Err(PakerError::PathNotAllowed);
        }

        let canonical = path
            .canonicalize()
            .map_err(|_| PakerError::PathNotAllowed)?;

        {
            let files = self.allowed_files.lock().expect("allowed_files lock");
            if files.iter().any(|f| f == &canonical) {
                return Self::ensure_regular_file(&canonical);
            }
        }

        if !self.is_under_scope(&canonical) {
            return Err(PakerError::PathNotAllowed);
        }

        Self::ensure_regular_file(&canonical)
    }

    /// Validate an export destination file path (parent directory must be in scope).
    pub fn validate_export_path(&self, file_path: &Path) -> Result<PathBuf, PakerError> {
        let parent = file_path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .ok_or(PakerError::PathNotAllowed)?;

        let mut current = parent;
        loop {
            if current.exists() {
                self.validate_dir_access(current)?;
                return Ok(file_path.to_path_buf());
            }
            current = current.parent().ok_or(PakerError::PathNotAllowed)?;
        }
    }

    fn ensure_regular_file(canonical: &Path) -> Result<PathBuf, PakerError> {
        let meta = fs::metadata(canonical).map_err(|_| PakerError::PathNotAllowed)?;
        #[cfg(unix)]
        if meta.is_symlink() {
            return Err(PakerError::PathNotAllowed);
        }
        if !meta.is_file() {
            return Err(PakerError::InvalidInput("Path is not a file".to_string()));
        }
        Ok(canonical.to_path_buf())
    }

    /// Expand scope for drag-drop uploads by registering each file and its parent folder.
    pub fn prepare_upload_paths(&self, paths: &[PathBuf]) -> Result<Vec<PathBuf>, PakerError> {
        let mut validated = Vec::with_capacity(paths.len());
        for path in paths {
            if let Some(parent) = path.parent() {
                self.register_picked_folder(parent);
            }
            let file = self.validate_file_access(path)?;
            self.register_file_paths(std::slice::from_ref(&file));
            validated.push(file);
        }
        Ok(validated)
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
pub async fn list_local_dir(app: AppHandle, path: String) -> Result<Vec<LocalEntry>, PakerError> {
    let scope = app.state::<LocalFsScope>();
    let dir = scope.validate_access(Path::new(&path))?;
    let read = fs::read_dir(&dir).map_err(|_| PakerError::PathNotAllowed)?;

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
pub async fn get_home_dir(app: AppHandle) -> Result<String, PakerError> {
    Ok(app
        .state::<LocalFsScope>()
        .home_dir()
        .to_string_lossy()
        .into_owned())
}

#[tauri::command]
pub async fn pick_local_folder(app: AppHandle) -> Result<Option<String>, PakerError> {
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
pub async fn get_parent_path(path: String) -> Result<Option<String>, PakerError> {
    let parent = Path::new(&path)
        .parent()
        .map(|p| p.to_string_lossy().into_owned());
    Ok(parent)
}

#[tauri::command]
pub async fn open_preview_file(app: AppHandle, path: String) -> Result<(), PakerError> {
    let cache_dir = preview_cache_dir(&app).map_err(|e| {
        tracing::warn!(error = %e, "preview cache dir unavailable");
        PakerError::Internal
    })?;
    let cache_dir = cache_dir.canonicalize().unwrap_or(cache_dir);

    let file = PathBuf::from(&path);
    let canonical = file
        .canonicalize()
        .map_err(|_| PakerError::PathNotAllowed)?;

    if !is_path_under_root(&canonical, &cache_dir) {
        tracing::warn!("blocked open_preview_file outside preview cache");
        return Err(PakerError::PathNotAllowed);
    }

    app.opener()
        .open_path(canonical.to_string_lossy().as_ref(), None::<&str>)
        .map_err(|e| {
            tracing::warn!(error = %e, "opener failed");
            PakerError::Internal
        })
}

fn format_unix_timestamp(secs: u64) -> String {
    let secs_i64 = secs as i64;
    let days = secs_i64 / 86400;
    let rem = secs_i64 % 86400;
    let hour = rem / 3600;
    let min = (rem % 3600) / 60;
    let sec = rem % 60;
    let (year, month, day) = days_to_ymd(days);

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hour, min, sec
    )
}

fn days_to_ymd(z: i64) -> (i64, i64, i64) {
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
        assert_eq!(
            allowed,
            scope
                .home_dir()
                .canonicalize()
                .unwrap_or_else(|_| scope.home_dir().to_path_buf())
        );
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
        assert!(matches!(
            scope.validate_access(&blocked),
            Err(PakerError::PathNotAllowed)
        ));
    }

    #[test]
    fn rejects_directory_as_file_access() {
        let scope = LocalFsScope::new().expect("scope");
        let home = scope.home_dir().to_path_buf();
        assert!(matches!(
            scope.validate_file_access(&home),
            Err(PakerError::InvalidInput(_)) | Err(PakerError::PathNotAllowed)
        ));
    }
}
