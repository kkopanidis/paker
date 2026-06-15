use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use crate::assistant::builders::{ActionKind, PartialError};
use crate::error::PakerError;
use crate::storage::paths;
use tauri::AppHandle;

const AUDIT_LOG_FILENAME: &str = "paker-audit.ndjson";
const ROTATION_SIZE_BYTES: u64 = 50 * 1024 * 1024; // 50 MB
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AuditOutcome {
    Executed,
    Rejected,
    ExpiredAbandoned,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEntry {
    pub ts: String,
    pub proposal_id: String,
    pub kind: ActionKind,
    pub outcome: AuditOutcome,
    pub connection_id: String,
    pub bucket: String,
    pub objects_affected: usize,
    pub bytes_affected: u64,
    pub errors: Vec<PartialError>,
    pub app_version: &'static str,
}

impl AuditEntry {
    pub fn new(
        proposal_id: String,
        kind: ActionKind,
        outcome: AuditOutcome,
        connection_id: String,
        bucket: String,
        objects_affected: usize,
        bytes_affected: u64,
        errors: Vec<PartialError>,
    ) -> Self {
        Self {
            ts: chrono::Utc::now().to_rfc3339(),
            proposal_id,
            kind,
            outcome,
            connection_id,
            bucket,
            objects_affected,
            bytes_affected,
            errors,
            app_version: APP_VERSION,
        }
    }
}

pub struct AuditLog {
    path: PathBuf,
}

impl AuditLog {
    pub fn new(app_handle: &AppHandle) -> Result<Self, PakerError> {
        let dir = paths::data_dir(app_handle).map_err(|e| {
            tracing::error!(error = %e, "failed to resolve audit log directory");
            PakerError::Internal
        })?;
        let path = dir.join(AUDIT_LOG_FILENAME);
        Ok(Self { path })
    }

    pub fn append(&self, entry: &AuditEntry) -> Result<(), PakerError> {
        self.rotate_if_needed()?;
        let mut line = serde_json::to_string(entry).map_err(|e| {
            tracing::error!(error = %e, "failed to serialise audit entry");
            PakerError::Internal
        })?;
        line.push('\n');

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|e| {
                tracing::error!(error = %e, path = %self.path.display(), "failed to open audit log");
                PakerError::Internal
            })?;

        file.write_all(line.as_bytes()).map_err(|e| {
            tracing::error!(error = %e, "failed to write audit log entry");
            PakerError::Internal
        })?;

        file.flush().map_err(|e| {
            tracing::error!(error = %e, "failed to flush audit log");
            PakerError::Internal
        })?;

        Ok(())
    }

    fn rotate_if_needed(&self) -> Result<(), PakerError> {
        if !self.path.exists() {
            return Ok(());
        }

        let size = fs::metadata(&self.path)
            .map(|m| m.len())
            .unwrap_or(0);

        if size > ROTATION_SIZE_BYTES {
            let ts = chrono::Utc::now().timestamp();
            let backup = self.path.with_file_name(format!(
                "paker-audit.{ts}.ndjson.bak"
            ));
            fs::rename(&self.path, &backup).map_err(|e| {
                tracing::error!(error = %e, "failed to rotate audit log");
                PakerError::Internal
            })?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_creates_valid_ndjson() {
        let dir = tempfile::tempdir().expect("tempdir");
        let log = AuditLog {
            path: dir.path().join(AUDIT_LOG_FILENAME),
        };

        for i in 0..3 {
            let entry = AuditEntry::new(
                format!("proposal-{i}"),
                ActionKind::DeleteByQuery,
                AuditOutcome::Executed,
                "conn-1".to_string(),
                "bucket".to_string(),
                i,
                i as u64 * 1024,
                vec![],
            );
            log.append(&entry).expect("append");
        }

        let contents = fs::read_to_string(log.path).expect("read");
        let lines: Vec<&str> = contents.lines().collect();
        assert_eq!(lines.len(), 3);
        for line in lines {
            let parsed: serde_json::Value =
                serde_json::from_str(line).expect("valid JSON line");
            assert!(parsed.get("proposalId").is_some());
            assert!(parsed.get("outcome").is_some());
        }
    }

    #[test]
    fn rotation_renames_when_over_size() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(AUDIT_LOG_FILENAME);

        // Write a file that is artificially "large" by writing > 50MB of data
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .open(&path)
            .unwrap();
        // Write just enough bytes to trigger rotation
        let big_data = vec![b'X'; (ROTATION_SIZE_BYTES + 1) as usize];
        file.write_all(&big_data).unwrap();
        drop(file);

        let log = AuditLog { path: path.clone() };
        log.rotate_if_needed().expect("rotate");

        assert!(!path.exists(), "original file should be renamed");
        let backups: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .ends_with(".ndjson.bak")
            })
            .collect();
        assert_eq!(backups.len(), 1);
    }
}
