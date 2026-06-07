use anyhow::{anyhow, Result};
use std::path::{Component, Path, PathBuf};

/// Reject S3 object keys that would escape `save_dir` when joined as a local path.
pub fn sanitize_object_key_for_local_path(save_dir: &Path, key: &str) -> Result<PathBuf> {
    if key.is_empty() {
        return Err(anyhow!("object key is empty"));
    }

    if key.starts_with('/') || key.starts_with('\\') {
        return Err(anyhow!("object key must be relative (got absolute path)"));
    }

    if has_windows_drive_prefix(key) {
        return Err(anyhow!("object key must not contain a Windows drive prefix"));
    }

    let relative = normalize_object_key_components(key)?;
    let joined = save_dir.join(&relative);
    ensure_stays_under(save_dir, &joined)?;
    Ok(joined)
}

pub fn local_dest_path(save_dir: &Path, key: &str) -> Result<PathBuf> {
    sanitize_object_key_for_local_path(save_dir, key)
}

/// Validate an S3 key segment used for create/rename/join operations.
pub fn validate_s3_key_segment(segment: &str) -> Result<()> {
    let segment = segment.trim();
    if segment.is_empty() {
        return Err(anyhow!("name is empty"));
    }
    if segment.contains("..") {
        return Err(anyhow!("name must not contain '..'"));
    }
    if segment.starts_with('/') || segment.starts_with('\\') {
        return Err(anyhow!("name must be relative"));
    }
    if segment.contains('\0') || segment.chars().any(|c| c.is_control()) {
        return Err(anyhow!("name contains invalid characters"));
    }
    if has_windows_drive_prefix(segment) {
        return Err(anyhow!("name must not contain a Windows drive prefix"));
    }
    Ok(())
}

/// Validate a full S3 object key for mutations.
pub fn validate_s3_object_key(key: &str) -> Result<()> {
    if key.is_empty() {
        return Err(anyhow!("object key is empty"));
    }
    if key.starts_with('/') || key.starts_with('\\') {
        return Err(anyhow!("object key must be relative"));
    }
    if key.contains('\0') || key.chars().any(|c| c.is_control()) {
        return Err(anyhow!("object key contains invalid characters"));
    }
    let normalized = key.replace('\\', "/");
    for part in normalized.split('/') {
        if part == ".." {
            return Err(anyhow!("object key must not contain '..'"));
        }
        if !part.is_empty() {
            validate_s3_key_segment(part)?;
        }
    }
    Ok(())
}

fn has_windows_drive_prefix(key: &str) -> bool {
    let mut chars = key.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_alphabetic() {
        return false;
    }
    matches!(chars.next(), Some(':'))
}

fn normalize_object_key_components(key: &str) -> Result<PathBuf> {
    let normalized = key.replace('\\', "/");
    let path = Path::new(&normalized);

    let mut relative = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                return Err(anyhow!("object key contains path traversal (..)"));
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(anyhow!("object key must be relative"));
            }
            Component::Normal(part) => {
                let part = part.to_string_lossy();
                if has_windows_drive_prefix(&part) {
                    return Err(anyhow!(
                        "object key must not contain a Windows drive prefix"
                    ));
                }
                if part == ".." {
                    return Err(anyhow!("object key contains path traversal (..)"));
                }
                relative.push(part.as_ref());
            }
            Component::CurDir => {}
        }
    }

    if relative.as_os_str().is_empty() {
        return Err(anyhow!("object key has no file name"));
    }

    Ok(relative)
}

fn ensure_stays_under(base: &Path, candidate: &Path) -> Result<()> {
    let base_norm = normalize_lexical(base);
    let candidate_norm = normalize_lexical(candidate);

    if !is_lexically_under(&base_norm, &candidate_norm) {
        return Err(anyhow!("resolved path escapes save directory"));
    }

    if let Ok(base_canon) = base.canonicalize() {
        if let Ok(candidate_canon) = candidate.canonicalize() {
            if !candidate_canon.starts_with(&base_canon) {
                return Err(anyhow!("resolved path escapes save directory"));
            }
        }
    }

    Ok(())
}

fn normalize_lexical(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::Normal(part) => normalized.push(part),
            Component::ParentDir => {
                let _ = normalized.pop();
            }
            Component::CurDir => {}
        }
    }
    normalized
}

pub fn is_path_under_root(path: &Path, root: &Path) -> bool {
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    path == root || path.starts_with(&root)
}

fn is_lexically_under(base: &Path, candidate: &Path) -> bool {
    if base == candidate {
        return true;
    }

    let base_iter = base.components().peekable();
    let mut candidate_iter = candidate.components();

    for base_comp in base_iter {
        match candidate_iter.next() {
            Some(candidate_comp) if base_comp == candidate_comp => {}
            _ => return false,
        }
    }

    candidate_iter.next().is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use uuid::Uuid;

    static TEST_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn test_save_dir(name: &str) -> PathBuf {
        let id = TEST_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("paker-path-safety-{name}-{id}-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("create test dir");
        dir
    }

    #[test]
    fn allows_nested_relative_key() {
        let dir = test_save_dir("nested");
        let dest = sanitize_object_key_for_local_path(&dir, "photos/2024/cat.jpg").expect("valid key");
        assert_eq!(dest, dir.join("photos/2024/cat.jpg"));
    }

    #[test]
    fn normalizes_backslashes() {
        let dir = test_save_dir("slashes");
        let dest = sanitize_object_key_for_local_path(&dir, "photos\\2024\\cat.jpg").expect("valid");
        assert_eq!(dest, dir.join("photos/2024/cat.jpg"));
    }

    #[test]
    fn rejects_parent_dir_segments() {
        let dir = test_save_dir("parent-segments");
        let err = sanitize_object_key_for_local_path(&dir, "foo/../../secret.txt").expect_err("traversal");
        assert!(err.to_string().contains("path traversal"));
    }

    #[test]
    fn rejects_leading_parent_dir() {
        let dir = test_save_dir("leading-parent");
        let err = sanitize_object_key_for_local_path(&dir, "../secret.txt").expect_err("traversal");
        assert!(err.to_string().contains("path traversal"));
    }

    #[test]
    fn rejects_absolute_unix_path() {
        let dir = test_save_dir("absolute");
        let err = sanitize_object_key_for_local_path(&dir, "/etc/passwd").expect_err("absolute");
        assert!(err.to_string().contains("relative"));
    }

    #[test]
    fn rejects_windows_drive_prefix() {
        let dir = test_save_dir("drive");
        let err = sanitize_object_key_for_local_path(&dir, "C:\\Windows\\win.ini").expect_err("drive");
        assert!(err.to_string().contains("Windows drive"));
    }

    #[test]
    fn rejects_empty_key() {
        let dir = test_save_dir("empty");
        let err = sanitize_object_key_for_local_path(&dir, "").expect_err("empty");
        assert!(err.to_string().contains("empty"));
    }

    #[test]
    fn rejects_invalid_s3_key_segment() {
        assert!(validate_s3_key_segment("..").is_err());
        assert!(validate_s3_key_segment("").is_err());
        assert!(validate_s3_object_key("foo/../bar").is_err());
    }

    #[test]
    fn canonical_check_blocks_symlink_escape() {
        let dir = test_save_dir("symlink-base");
        let outside = test_save_dir("symlink-outside");
        let secret = outside.join("secret.txt");
        fs::write(&secret, b"secret").expect("write secret");

        #[cfg(unix)]
        {
            let link = dir.join("escape");
            std::os::unix::fs::symlink(&outside, &link).expect("symlink");
            let err =
                sanitize_object_key_for_local_path(&dir, "escape/secret.txt").expect_err("symlink escape");
            assert!(err.to_string().contains("escapes save directory"));
        }

        #[cfg(not(unix))]
        {
            let _ = (dir, outside, secret);
        }
    }
}
