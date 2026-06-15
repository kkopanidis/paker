use std::collections::HashMap;

use chrono::Utc;

use crate::assistant::builders::{
    ActionKind, ActionProposal, MAX_PREVIEW_ITEMS, ProposalItem,
};
use crate::assistant::hmac_token;
use crate::assistant::policy::{self, PolicyContext};
use crate::assistant::proposal_store::{
    ProposalEntry, ProposalStatus, ProposalStore, new_proposal_id, proposal_expires_at,
};
use crate::assistant::query::IndexQuery;
use crate::assistant::templates::{CliCommandSuggestion, CliGenerateInput, generate_cli_commands};
use crate::error::PakerError;
use crate::storage::ObjectCacheManager;

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum SyncMode {
    AddOnly,
    Mirror,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncPlanInput {
    pub connection_id: String,
    pub bucket: String,
    pub source_prefix: String,
    pub dest_prefix: String,
    pub mode: SyncMode,
    pub generate_cli: bool,
}

pub async fn build(
    input: &SyncPlanInput,
    cache: &ObjectCacheManager,
    store: &ProposalStore,
    hmac_key: &hmac_token::HmacKey,
) -> Result<ActionProposal, PakerError> {
    let source_query = IndexQuery {
        prefix: if input.source_prefix.is_empty() {
            None
        } else {
            Some(input.source_prefix.clone())
        },
        limit: 10_000,
        ..Default::default()
    };
    let dest_query = IndexQuery {
        prefix: if input.dest_prefix.is_empty() {
            None
        } else {
            Some(input.dest_prefix.clone())
        },
        limit: 10_000,
        ..Default::default()
    };

    let source_objects = cache
        .query_bucket_index(&input.connection_id, &input.bucket, &source_query)
        .map_err(|_| PakerError::Internal)?;
    let dest_objects = cache
        .query_bucket_index(&input.connection_id, &input.bucket, &dest_query)
        .map_err(|_| PakerError::Internal)?;

    // Build relative-key maps
    let src_map: HashMap<String, &crate::storage::IndexedObject> = source_objects
        .iter()
        .map(|o| {
            let rel = strip_prefix(&o.key, &input.source_prefix);
            (rel, o)
        })
        .collect();

    let dest_map: HashMap<String, &crate::storage::IndexedObject> = dest_objects
        .iter()
        .map(|o| {
            let rel = strip_prefix(&o.key, &input.dest_prefix);
            (rel, o)
        })
        .collect();

    let mut preview_items: Vec<ProposalItem> = Vec::new();
    let mut total_bytes = 0u64;

    // to_add: in source but not in dest
    for (rel, src) in &src_map {
        if !dest_map.contains_key(rel.as_str()) {
            total_bytes += src.size.max(0) as u64;
            if preview_items.len() < MAX_PREVIEW_ITEMS {
                preview_items.push(ProposalItem {
                    key: src.key.clone(),
                    size_bytes: src.size.max(0) as u64,
                    storage_class: src.storage_class.clone(),
                    action_description: format!("Add → {}{rel}", input.dest_prefix),
                    metadata: None,
                });
            }
        }
    }

    // to_update: in both, source is newer
    for (rel, src) in &src_map {
        if let Some(dst) = dest_map.get(rel.as_str()) {
            let src_newer = match (&src.last_modified, &dst.last_modified) {
                (Some(s), Some(d)) => s > d,
                _ => false,
            };
            if src_newer {
                total_bytes += src.size.max(0) as u64;
                if preview_items.len() < MAX_PREVIEW_ITEMS {
                    preview_items.push(ProposalItem {
                        key: src.key.clone(),
                        size_bytes: src.size.max(0) as u64,
                        storage_class: src.storage_class.clone(),
                        action_description: format!("Update → {}{rel}", input.dest_prefix),
                        metadata: None,
                    });
                }
            }
        }
    }

    // to_delete: in dest but not in source (mirror mode only)
    if matches!(input.mode, SyncMode::Mirror) {
        for (rel, dst) in &dest_map {
            if !src_map.contains_key(rel.as_str()) {
                if preview_items.len() < MAX_PREVIEW_ITEMS {
                    preview_items.push(ProposalItem {
                        key: dst.key.clone(),
                        size_bytes: dst.size.max(0) as u64,
                        storage_class: dst.storage_class.clone(),
                        action_description: format!("Delete {}", dst.key),
                        metadata: None,
                    });
                }
            }
        }
    }

    let total_affected = preview_items.len();

    let policy_ctx = PolicyContext {
        kind: ActionKind::SyncPlan,
        affected_count: total_affected,
        has_glacier: false,
        bucket_versioned: false,
        index_age_secs: Some(0),
    };
    let policy_result = policy::check(&policy_ctx);
    if !policy_result.is_clean() {
        return Err(PakerError::PolicyViolation(
            policy_result.violations.join("; "),
        ));
    }

    let cli_suggestions: Option<Vec<CliCommandSuggestion>> = if input.generate_cli {
        let cli_input = CliGenerateInput {
            tool: None,
            connection_id: input.connection_id.clone(),
            connection_name: None,
            endpoint: None,
            bucket: input.bucket.clone(),
            prefix: Some(input.source_prefix.clone()),
            keys: vec![],
        };
        Some(generate_cli_commands(&cli_input))
    } else {
        None
    };

    let now = Utc::now();
    let id = new_proposal_id();
    let expires_at = proposal_expires_at(now);
    let kind_str = ActionKind::SyncPlan.to_string();
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
        kind: ActionKind::SyncPlan,
        connection_id: input.connection_id.clone(),
        bucket: input.bucket.clone(),
        payload: serde_json::json!({
            "sourcePrefix": input.source_prefix,
            "destPrefix": input.dest_prefix,
            "mode": input.mode,
        }),
        token: token.clone(),
        status: ProposalStatus::Pending,
        created_at: now,
        expires_at,
    };
    store.insert(entry);

    Ok(ActionProposal {
        id,
        kind: ActionKind::SyncPlan,
        connection_id: input.connection_id.clone(),
        bucket: input.bucket.clone(),
        preview_items,
        total_affected,
        total_bytes,
        warnings: policy_result.warning_messages(),
        token,
        expires_at: expires_at.to_rfc3339(),
        cli_suggestions,
    })
}

fn strip_prefix<'a>(key: &'a str, prefix: &str) -> String {
    if prefix.is_empty() {
        return key.to_string();
    }
    key.strip_prefix(prefix).unwrap_or(key).to_string()
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
        let dir =
            std::env::temp_dir().join(format!("paker-sync-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        ObjectCacheManager::open(dir.join("index.db")).expect("open cache")
    }

    fn obj(key: &str, last_modified: &str) -> IndexedObject {
        IndexedObject {
            key: key.to_string(),
            size: 100,
            last_modified: Some(last_modified.to_string()),
            etag: None,
            storage_class: None,
        }
    }

    #[tokio::test]
    async fn sync_plan_computes_add_update_delete() {
        let cache = temp_cache();
        // Source: 5 keys
        let source = vec![
            obj("src/a.txt", "2024-06-01"),
            obj("src/b.txt", "2024-06-01"),
            obj("src/c.txt", "2024-07-01"), // newer than dest
            obj("src/d.txt", "2024-06-01"),
            obj("src/e.txt", "2024-06-01"),
        ];
        // Dest: 3 overlapping + 1 extra (f.txt only in dest)
        let dest = vec![
            obj("dst/a.txt", "2024-06-01"),
            obj("dst/b.txt", "2024-06-01"),
            obj("dst/c.txt", "2024-05-01"), // older → update
            obj("dst/f.txt", "2024-06-01"), // only in dest → delete (mirror)
        ];

        cache
            .upsert_indexed_objects_batch("conn", "bucket", &source)
            .unwrap();
        cache
            .upsert_indexed_objects_batch("conn", "bucket", &dest)
            .unwrap();

        let store = ProposalStore::default();
        let key = HmacKey::generate();

        let input = SyncPlanInput {
            connection_id: "conn".to_string(),
            bucket: "bucket".to_string(),
            source_prefix: "src/".to_string(),
            dest_prefix: "dst/".to_string(),
            mode: SyncMode::Mirror,
            generate_cli: false,
        };

        let proposal = build(&input, &cache, &store, &key)
            .await
            .expect("build");

        let adds: usize = proposal
            .preview_items
            .iter()
            .filter(|i| i.action_description.starts_with("Add"))
            .count();
        let updates: usize = proposal
            .preview_items
            .iter()
            .filter(|i| i.action_description.starts_with("Update"))
            .count();
        let deletes: usize = proposal
            .preview_items
            .iter()
            .filter(|i| i.action_description.starts_with("Delete"))
            .count();

        // d.txt and e.txt → to_add (2)
        assert_eq!(adds, 2, "expected 2 adds");
        // c.txt → to_update (1)
        assert_eq!(updates, 1, "expected 1 update");
        // f.txt → to_delete (1, mirror mode)
        assert_eq!(deletes, 1, "expected 1 delete");
    }
}
