use std::collections::HashMap;

use chrono::Utc;

use crate::assistant::builders::{
    ActionKind, ActionProposal, ExecutionResult, MAX_PREVIEW_ITEMS, PartialError, ProposalItem,
};
use crate::assistant::hmac_token;
use crate::assistant::policy::{self, PolicyContext};
use crate::assistant::proposal_store::{
    ProposalEntry, ProposalStatus, ProposalStore, new_proposal_id, proposal_expires_at,
};
use crate::assistant::query::IndexQuery;
use crate::error::PakerError;
use crate::storage::ObjectCacheManager;

const MAX_PATTERN_LEN: usize = 1024;

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenamePatternInput {
    pub connection_id: String,
    pub bucket: String,
    pub source_pattern: String,
    pub dest_template: String,
    pub copy_only: bool,
    pub query: Option<IndexQuery>,
}

/// Split a glob pattern at `*` wildcards and extract the literal parts.
/// Returns `None` if the key does not match.
pub fn extract_captures(pattern: &str, key: &str) -> Option<Vec<String>> {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        // No wildcards: exact match required
        if pattern == key {
            return Some(vec![key.to_string()]);
        }
        return None;
    }

    let first = parts[0];
    if !key.starts_with(first) {
        return None;
    }

    let mut remaining = &key[first.len()..];
    let mut captures: Vec<String> = Vec::new();

    for (i, &part) in parts[1..].iter().enumerate() {
        let is_last = i == parts.len() - 2;
        if is_last {
            // The last capture extends to the end minus the trailing literal
            if part.is_empty() {
                captures.push(remaining.to_string());
                remaining = "";
            } else if let Some(pos) = remaining.rfind(part) {
                // Ensure the rest after pos matches exactly
                if &remaining[pos..] == part {
                    captures.push(remaining[..pos].to_string());
                    remaining = "";
                } else {
                    return None;
                }
            } else {
                return None;
            }
        } else if part.is_empty() {
            // Two consecutive `*` — capture up to the next non-empty literal
            // Find the next non-empty literal to know where this capture ends
            let next_literal = parts[i + 2];
            if next_literal.is_empty() {
                // All remaining goes to this capture; subsequent logic handles next
                captures.push(remaining.to_string());
                remaining = "";
            } else if let Some(pos) = remaining.find(next_literal) {
                captures.push(remaining[..pos].to_string());
                // Don't advance; let next iteration handle the literal
            } else {
                return None;
            }
        } else if let Some(pos) = remaining.find(part) {
            captures.push(remaining[..pos].to_string());
            remaining = &remaining[pos + part.len()..];
        } else {
            return None;
        }
    }

    if !remaining.is_empty() {
        return None;
    }

    Some(captures)
}

/// Replace `{0}` (full key) and `{1}`, `{2}`, ... (wildcard captures) in the template.
pub fn substitute_template(full_key: &str, captures: &[String], template: &str) -> String {
    let mut result = template.replace("{0}", full_key);
    for (i, cap) in captures.iter().enumerate() {
        result = result.replace(&format!("{{{}}}", i + 1), cap);
    }
    result
}

fn validate_input(input: &RenamePatternInput) -> Result<(), PakerError> {
    if input.source_pattern.len() > MAX_PATTERN_LEN || input.dest_template.len() > MAX_PATTERN_LEN
    {
        return Err(PakerError::InvalidInput(
            "Pattern or template exceeds maximum length of 1024 characters".to_string(),
        ));
    }
    if input.source_pattern.contains('\0') || input.dest_template.contains('\0') {
        return Err(PakerError::InvalidInput(
            "Pattern or template must not contain null bytes".to_string(),
        ));
    }
    Ok(())
}

pub async fn build(
    input: &RenamePatternInput,
    cache: &ObjectCacheManager,
    store: &ProposalStore,
    hmac_key: &hmac_token::HmacKey,
    index_age_secs: Option<u64>,
) -> Result<ActionProposal, PakerError> {
    validate_input(input)?;

    let objects = if let Some(q) = &input.query {
        cache
            .query_bucket_index(&input.connection_id, &input.bucket, q)
            .map_err(|e| {
                tracing::error!(error = %e, "rename_pattern: query_bucket_index failed");
                PakerError::Internal
            })?
    } else {
        let all_query = IndexQuery {
            limit: 10_000,
            ..Default::default()
        };
        cache
            .query_bucket_index(&input.connection_id, &input.bucket, &all_query)
            .map_err(|_| PakerError::Internal)?
    };

    // Filter by source pattern and compute dest keys
    let pairs: Vec<(String, String, i64, Option<String>)> = objects
        .iter()
        .filter_map(|o| {
            let caps = extract_captures(&input.source_pattern, &o.key)?;
            let dest = substitute_template(&o.key, &caps, &input.dest_template);
            if dest == o.key {
                return None; // No-op rename
            }
            Some((o.key.clone(), dest, o.size, o.storage_class.clone()))
        })
        .collect();

    let has_glacier = pairs.iter().any(|(_, _, _, sc)| {
        sc.as_deref()
            .map(|s| {
                let up = s.to_uppercase();
                up.contains("GLACIER") || up.contains("DEEP_ARCHIVE")
            })
            .unwrap_or(false)
    });

    let policy_ctx = PolicyContext {
        kind: ActionKind::RenamePattern,
        affected_count: pairs.len(),
        has_glacier,
        bucket_versioned: false,
        index_age_secs,
    };
    let policy_result = policy::check(&policy_ctx);
    if !policy_result.is_clean() {
        return Err(PakerError::PolicyViolation(
            policy_result.violations.join("; "),
        ));
    }

    let total_bytes: u64 = pairs.iter().map(|(_, _, sz, _)| (*sz).max(0) as u64).sum();
    let total_affected = pairs.len();

    let preview_items: Vec<ProposalItem> = pairs
        .iter()
        .take(MAX_PREVIEW_ITEMS)
        .map(|(src, dest, sz, sc)| {
            let desc = if input.copy_only {
                format!("Copy → s3://{}/{dest}", input.bucket)
            } else {
                format!("Rename: {src} → {dest}")
            };
            let mut metadata = HashMap::new();
            metadata.insert("destKey".to_string(), dest.clone());
            ProposalItem {
                key: src.clone(),
                size_bytes: (*sz).max(0) as u64,
                storage_class: sc.clone(),
                action_description: desc,
                metadata: Some(metadata),
            }
        })
        .collect();

    let stored_pairs: Vec<(String, String)> = pairs
        .iter()
        .map(|(s, d, _, _)| (s.clone(), d.clone()))
        .collect();

    let now = Utc::now();
    let id = new_proposal_id();
    let expires_at = proposal_expires_at(now);
    let kind_str = ActionKind::RenamePattern.to_string();
    let token = hmac_token::sign(
        hmac_key,
        &id,
        &input.connection_id,
        &input.bucket,
        &kind_str,
        now.timestamp(),
    );

    let entry = ProposalEntry {
        id: id.clone(),
        kind: ActionKind::RenamePattern,
        connection_id: input.connection_id.clone(),
        bucket: input.bucket.clone(),
        payload: serde_json::json!({
            "pairs": stored_pairs,
            "copyOnly": input.copy_only,
        }),
        token: token.clone(),
        status: ProposalStatus::Pending,
        created_at: now,
        expires_at,
    };
    store.insert(entry);

    Ok(ActionProposal {
        id,
        kind: ActionKind::RenamePattern,
        connection_id: input.connection_id.clone(),
        bucket: input.bucket.clone(),
        preview_items,
        total_affected,
        total_bytes,
        warnings: policy_result.warning_messages(),
        token,
        expires_at: expires_at.to_rfc3339(),
        cli_suggestions: None,
    })
}

pub async fn execute(
    entry: &ProposalEntry,
    app_handle: &tauri::AppHandle,
) -> Result<ExecutionResult, PakerError> {
    let copy_only = entry
        .payload
        .get("copyOnly")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let pairs: Vec<(String, String)> = entry
        .payload
        .get("pairs")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    let total = pairs.len();
    let client =
        crate::s3::client::build_client_for_id(app_handle, &entry.connection_id).await?;

    let mut errors: Vec<PartialError> = vec![];
    let mut copied_srcs: Vec<String> = vec![];
    let mut done = 0usize;

    use tauri::Emitter;

    for (src, dest) in &pairs {
        let copy_source = format!("{}/{}", entry.bucket, src);
        let copy_result = client
            .copy_object()
            .bucket(&entry.bucket)
            .key(dest)
            .copy_source(&copy_source)
            .send()
            .await;

        match copy_result {
            Ok(_) => {
                copied_srcs.push(src.clone());
                done += 1;
            }
            Err(e) => {
                errors.push(PartialError {
                    key: src.clone(),
                    message: e.to_string(),
                });
            }
        }

        let _ = app_handle.emit(
            "proposal://progress",
            serde_json::json!({
                "proposalId": entry.id,
                "done": done,
                "total": total,
                "phase": "copying",
            }),
        );
    }

    if !copy_only && !copied_srcs.is_empty() {
        for chunk in copied_srcs.chunks(1000) {
            if let Err(e) =
                crate::s3::operations::delete_objects_batch(&client, &entry.bucket, chunk).await
            {
                for key in chunk {
                    errors.push(PartialError {
                        key: key.clone(),
                        message: format!("copy succeeded but delete failed: {e}"),
                    });
                }
            }
        }
    }

    let _ = app_handle.emit(
        "proposal://progress",
        serde_json::json!({
            "proposalId": entry.id,
            "done": done,
            "total": total,
            "phase": "complete",
        }),
    );

    Ok(ExecutionResult {
        proposal_id: entry.id.clone(),
        kind: ActionKind::RenamePattern,
        objects_affected: done,
        bytes_affected: 0,
        errors,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_captures_simple_glob() {
        let caps = extract_captures("logs/*/file.gz", "logs/2024/file.gz").unwrap();
        assert_eq!(caps, vec!["2024"]);
    }

    #[test]
    fn extract_captures_two_wildcards() {
        let caps = extract_captures("logs/*/*/*.gz", "logs/2024/jan/data.gz").unwrap();
        assert_eq!(caps, vec!["2024", "jan", "data"]);
    }

    #[test]
    fn extract_captures_no_match() {
        assert!(extract_captures("logs/*.gz", "photos/x.gz").is_none());
    }

    #[test]
    fn substitute_template_positional() {
        let caps = vec!["2024".to_string(), "jan".to_string()];
        let result = substitute_template("logs/2024/jan.gz", &caps, "archive/{1}/{2}.gz");
        assert_eq!(result, "archive/2024/jan.gz");
    }

    #[test]
    fn substitute_template_zero_is_full_key() {
        let caps = vec!["2024".to_string()];
        let result = substitute_template("logs/2024.gz", &caps, "backup/{0}");
        assert_eq!(result, "backup/logs/2024.gz");
    }
}
