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
use crate::s3::operations::delete_objects_batch;
use crate::storage::ObjectCacheManager;

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteByQueryInput {
    pub connection_id: String,
    pub bucket: String,
    pub query: IndexQuery,
    pub dry_run: bool,
}

pub async fn build(
    input: &DeleteByQueryInput,
    cache: &ObjectCacheManager,
    store: &ProposalStore,
    hmac_key: &hmac_token::HmacKey,
    index_age_secs: Option<u64>,
) -> Result<ActionProposal, PakerError> {
    let objects = cache
        .query_bucket_index(&input.connection_id, &input.bucket, &input.query)
        .map_err(|e| {
            tracing::error!(error = %e, "delete_by_query: query_bucket_index failed");
            PakerError::Internal
        })?;

    let has_glacier = objects.iter().any(|o| {
        o.storage_class
            .as_deref()
            .map(|s| {
                let up = s.to_uppercase();
                up.contains("GLACIER") || up.contains("DEEP_ARCHIVE")
            })
            .unwrap_or(false)
    });

    let policy_ctx = PolicyContext {
        kind: ActionKind::DeleteByQuery,
        affected_count: objects.len(),
        has_glacier,
        bucket_versioned: false, // caller may enrich later; conservative default
        index_age_secs,
    };
    let policy_result = policy::check(&policy_ctx);
    if !policy_result.is_clean() {
        return Err(PakerError::PolicyViolation(
            policy_result.violations.join("; "),
        ));
    }

    let total_bytes: u64 = objects.iter().map(|o| o.size.max(0) as u64).sum();
    let total_affected = objects.len();

    let preview_items: Vec<ProposalItem> = objects
        .iter()
        .take(MAX_PREVIEW_ITEMS)
        .map(|o| ProposalItem {
            key: o.key.clone(),
            size_bytes: o.size.max(0) as u64,
            storage_class: o.storage_class.clone(),
            action_description: format!("Delete s3://{}/{}", input.bucket, o.key),
            metadata: None,
        })
        .collect();

    // Build payload for storage (full key list)
    let keys: Vec<String> = objects.iter().map(|o| o.key.clone()).collect();

    let now = Utc::now();
    let id = new_proposal_id();
    let expires_at = proposal_expires_at(now);
    let kind_str = ActionKind::DeleteByQuery.to_string();
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
        kind: ActionKind::DeleteByQuery,
        connection_id: input.connection_id.clone(),
        bucket: input.bucket.clone(),
        payload: serde_json::json!({
            "keys": keys,
            "dryRun": input.dry_run,
            "query": input.query,
        }),
        token: token.clone(),
        status: ProposalStatus::Pending,
        created_at: now,
        expires_at,
    };
    store.insert(entry);

    Ok(ActionProposal {
        id,
        kind: ActionKind::DeleteByQuery,
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
    cache: &ObjectCacheManager,
) -> Result<ExecutionResult, PakerError> {
    let dry_run = entry
        .payload
        .get("dryRun")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let stored_keys: Vec<String> = entry
        .payload
        .get("keys")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    let query: IndexQuery = entry
        .payload
        .get("query")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    // Re-query to intersect with live index (safety measure)
    let live_objects = cache
        .query_bucket_index(&entry.connection_id, &entry.bucket, &query)
        .unwrap_or_default();
    let live_keys: std::collections::HashSet<String> =
        live_objects.iter().map(|o| o.key.clone()).collect();
    let stored_set: std::collections::HashSet<String> = stored_keys.into_iter().collect();

    let keys_to_delete: Vec<String> = stored_set.intersection(&live_keys).cloned().collect();
    let total = keys_to_delete.len();
    let total_bytes: u64 = live_objects
        .iter()
        .filter(|o| live_keys.contains(&o.key))
        .map(|o| o.size.max(0) as u64)
        .sum();

    if dry_run || total == 0 {
        emit_progress(app_handle, &entry.id, total, total, "complete");
        return Ok(ExecutionResult {
            proposal_id: entry.id.clone(),
            kind: ActionKind::DeleteByQuery,
            objects_affected: 0,
            bytes_affected: 0,
            errors: vec![],
        });
    }

    let client = crate::s3::client::build_client_for_id(app_handle, &entry.connection_id).await?;
    let mut errors: Vec<PartialError> = vec![];
    let mut done = 0usize;

    for chunk in keys_to_delete.chunks(1000) {
        match delete_objects_batch(&client, &entry.bucket, chunk).await {
            Ok(()) => done += chunk.len(),
            Err(e) => {
                for key in chunk {
                    errors.push(PartialError {
                        key: key.clone(),
                        message: e.to_string(),
                    });
                }
            }
        }
        emit_progress(app_handle, &entry.id, done, total, "deleting");
    }

    emit_progress(app_handle, &entry.id, done, total, "complete");

    Ok(ExecutionResult {
        proposal_id: entry.id.clone(),
        kind: ActionKind::DeleteByQuery,
        objects_affected: done,
        bytes_affected: total_bytes,
        errors,
    })
}

fn emit_progress(app_handle: &tauri::AppHandle, proposal_id: &str, done: usize, total: usize, phase: &str) {
    use tauri::Emitter;
    let _ = app_handle.emit(
        "proposal://progress",
        serde_json::json!({
            "proposalId": proposal_id,
            "done": done,
            "total": total,
            "phase": phase,
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant::hmac_token::HmacKey;
    use crate::storage::bucket_index::IndexedObject;
    use crate::storage::object_cache::ObjectCacheManager;
    use std::fs;
    use uuid::Uuid;

    fn temp_cache() -> ObjectCacheManager {
        let dir = std::env::temp_dir()
            .join(format!("paker-del-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        ObjectCacheManager::open(dir.join("index.db")).expect("open cache")
    }

    fn seed_objects(cache: &ObjectCacheManager, conn: &str, bucket: &str, n: usize) {
        let objects: Vec<IndexedObject> = (0..n)
            .map(|i| IndexedObject {
                key: format!("key-{i}.txt"),
                size: (i * 100) as i64,
                last_modified: Some("2024-01-01T00:00:00Z".to_string()),
                etag: None,
                storage_class: None,
            })
            .collect();
        cache
            .upsert_indexed_objects_batch(conn, bucket, &objects)
            .unwrap();
    }

    #[tokio::test]
    async fn build_proposal_returns_preview_and_token() {
        let cache = temp_cache();
        seed_objects(&cache, "c1", "b1", 5);
        let store = ProposalStore::default();
        let key = HmacKey::generate();

        let input = DeleteByQueryInput {
            connection_id: "c1".to_string(),
            bucket: "b1".to_string(),
            query: IndexQuery { limit: 10_000, ..Default::default() },
            dry_run: false,
        };

        let proposal = build(&input, &cache, &store, &key, Some(60))
            .await
            .expect("build");

        assert_eq!(proposal.total_affected, 5);
        assert_eq!(proposal.preview_items.len(), 5);
        assert!(proposal.token.starts_with("v1."));
    }
}
