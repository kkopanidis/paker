use crate::assistant::audit_log::{AuditEntry, AuditLog, AuditOutcome};
use crate::assistant::builders::delete_by_query;
use crate::assistant::builders::rename_pattern;
use crate::assistant::builders::sync_plan;
use crate::assistant::builders::{ActionProposal, BuildProposalInput, ExecutionResult};
use crate::assistant::explain::{explain_error_code, ErrorExplanation};
use crate::assistant::hmac_token::{self, HmacKey};
use crate::assistant::proposal_store::{ProposalEntry, ProposalStore};
use crate::assistant::query::{parse_natural_language, IndexQuery, ParsedAssistantQuery};
use crate::assistant::reports::BucketReport;
use crate::assistant::templates::{generate_cli_commands, CliCommandSuggestion, CliGenerateInput};
use crate::commands::local_fs::LocalFsScope;
use crate::error::{into_ipc_error, PakerError};
use crate::storage::profiles::get_connection;
use crate::storage::{IndexedObject, ObjectCacheManager, QueryHistoryItem, VaultManager};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;

// ─── Phase 1: Query & analysis commands ──────────────────────────────────────

#[tauri::command]
pub fn assistant_parse_query(text: String) -> ParsedAssistantQuery {
    parse_natural_language(&text)
}

#[tauri::command]
pub fn assistant_get_model_status(app: AppHandle) -> Result<crate::assistant::llm::AssistantModelStatus, PakerError> {
    let data_dir = crate::storage::paths::data_dir(&app).map_err(into_ipc_error)?;
    let parser_loaded = app.try_state::<crate::assistant::llm::ModelHandle>().is_some();
    Ok(crate::assistant::llm::get_model_status(&data_dir, parser_loaded))
}

#[tauri::command]
pub async fn assistant_open_models_folder(app: AppHandle) -> Result<(), PakerError> {
    let dir = crate::storage::paths::models_dir(&app).map_err(into_ipc_error)?;
    app.opener()
        .open_path(dir.to_string_lossy().as_ref(), None::<&str>)
        .map_err(|e| {
            tracing::warn!(error = %e, "failed to open models folder");
            PakerError::Internal
        })
}

/// Parse a query using the regex parser, optionally upgraded by the LLM when
/// the `llm` feature is enabled and a model is loaded.  Falls back gracefully
/// to regex-only when no model is available.
#[tauri::command]
pub fn assistant_parse_query_llm(
    #[allow(unused_variables)] app: AppHandle,
    text: String,
) -> ParsedAssistantQuery {
    let regex_result = parse_natural_language(&text);

    #[cfg(feature = "llm")]
    {
        use crate::assistant::llm::gbnf_grammar::INDEX_QUERY_GBNF;
        use crate::assistant::llm::{merge_with_regex, run_grammar_parse, ModelHandle};
        use crate::assistant::query::ParseConfidence;

        if regex_result.confidence != ParseConfidence::High {
            if let Some(model) = app.try_state::<ModelHandle>() {
                if let Ok(raw_json) = run_grammar_parse(&model, &text, INDEX_QUERY_GBNF) {
                    if let Ok(index_query) = serde_json::from_str::<IndexQuery>(&raw_json) {
                        let llm_result = crate::assistant::llm::LlmParsedQuery {
                            index_query,
                            source: crate::assistant::llm::ParseSource::Llm,
                        };
                        return merge_with_regex(Some(llm_result), regex_result);
                    }
                }
            }
        }
    }

    regex_result
}

#[tauri::command]
pub async fn assistant_run_index_query(
    app: AppHandle,
    connection_id: String,
    bucket: String,
    query: IndexQuery,
    raw_text: Option<String>,
    summary: Option<String>,
    confidence: Option<String>,
) -> Result<Vec<IndexedObject>, PakerError> {
    let cache = app.state::<ObjectCacheManager>();
    let meta = cache.get_bucket_index_meta(&connection_id, &bucket);
    if meta.is_none() || meta.as_ref().is_some_and(|m| m.object_count == 0) {
        return Err(PakerError::IndexNotReady);
    }

    let results = cache
        .query_bucket_index(&connection_id, &bucket, &query)
        .map_err(into_ipc_error)?;

    if let (Some(rt), Some(sum), Some(conf)) = (raw_text, summary, confidence) {
        let _ = cache.insert_query_history(
            &connection_id,
            &bucket,
            &rt,
            &sum,
            &conf,
            results.len(),
        );
    }

    Ok(results)
}

#[tauri::command]
pub async fn assistant_get_bucket_report(
    app: AppHandle,
    connection_id: String,
    bucket: String,
    top_n: Option<u32>,
) -> Result<BucketReport, PakerError> {
    let cache = app.state::<ObjectCacheManager>();
    let meta = cache.get_bucket_index_meta(&connection_id, &bucket);
    if meta.is_none() || meta.as_ref().is_some_and(|m| m.object_count == 0) {
        return Err(PakerError::IndexNotReady);
    }

    cache
        .build_bucket_report(&connection_id, &bucket, top_n.unwrap_or(10))
        .map_err(into_ipc_error)
}

#[tauri::command]
pub fn assistant_explain_error(code: String) -> ErrorExplanation {
    explain_error_code(&code)
}

#[tauri::command]
pub async fn assistant_generate_cli(
    app: AppHandle,
    mut input: CliGenerateInput,
) -> Result<Vec<CliCommandSuggestion>, PakerError> {
    let profile = get_connection(&app, &input.connection_id)
        .map_err(|_| PakerError::Internal)?
        .ok_or(PakerError::ConnectionNotFound)?;
    input.connection_name = Some(profile.name);
    input.endpoint = profile.endpoint;
    Ok(generate_cli_commands(&input))
}

// ─── History ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn assistant_query_history_list(
    app: AppHandle,
    connection_id: String,
    bucket: String,
    limit: Option<u32>,
) -> Result<Vec<QueryHistoryItem>, PakerError> {
    let cache = app.state::<ObjectCacheManager>();
    cache
        .list_query_history(&connection_id, &bucket, limit.unwrap_or(20))
        .map_err(into_ipc_error)
}

#[tauri::command]
pub async fn assistant_query_history_clear(
    app: AppHandle,
    connection_id: String,
    bucket: String,
) -> Result<(), PakerError> {
    let cache = app.state::<ObjectCacheManager>();
    cache
        .clear_query_history(&connection_id, &bucket)
        .map_err(into_ipc_error)
}

#[tauri::command]
pub async fn assistant_query_history_insert(
    app: AppHandle,
    connection_id: String,
    bucket: String,
    raw_text: String,
    summary: String,
    confidence: String,
    result_count: u32,
) -> Result<(), PakerError> {
    let cache = app.state::<ObjectCacheManager>();
    cache
        .insert_query_history(
            &connection_id,
            &bucket,
            &raw_text,
            &summary,
            &confidence,
            result_count as usize,
        )
        .map_err(into_ipc_error)
}

// ─── Pack export ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExportFormat {
    Csv,
    Json,
    Clipboard,
}

fn build_export_content(objects: &[IndexedObject], keys_only: &[String], format: &ExportFormat) -> String {
    match format {
        ExportFormat::Clipboard => keys_only.join("\n"),
        ExportFormat::Json => serde_json::to_string_pretty(keys_only).unwrap_or_default(),
        ExportFormat::Csv => {
            let mut csv = String::from("key,size,last_modified,storage_class\n");
            for obj in objects {
                csv.push_str(&csv_escape(&obj.key));
                csv.push(',');
                csv.push_str(&obj.size.to_string());
                csv.push(',');
                csv.push_str(&csv_escape(obj.last_modified.as_deref().unwrap_or("")));
                csv.push(',');
                csv.push_str(&csv_escape(obj.storage_class.as_deref().unwrap_or("")));
                csv.push('\n');
            }
            let indexed_keys: std::collections::HashSet<&str> =
                objects.iter().map(|o| o.key.as_str()).collect();
            for key in keys_only {
                if !indexed_keys.contains(key.as_str()) {
                    csv.push_str(&csv_escape(key));
                    csv.push_str(",,,\n");
                }
            }
            csv
        }
    }
}

fn csv_escape(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

#[tauri::command]
pub async fn assistant_pack_export(
    app: AppHandle,
    connection_id: String,
    bucket: String,
    keys: Vec<String>,
    format: ExportFormat,
    save_path: Option<String>,
) -> Result<String, PakerError> {
    let cache = app.state::<ObjectCacheManager>();

    let objects = if matches!(format, ExportFormat::Clipboard | ExportFormat::Json) {
        vec![]
    } else {
        cache
            .get_objects_by_keys(&connection_id, &bucket, &keys)
            .map_err(into_ipc_error)?
    };

    let content = build_export_content(&objects, &keys, &format);

    if matches!(format, ExportFormat::Clipboard) {
        let _ = cache.log_pack_export(&connection_id, &bucket, None, keys.len());
        return Ok(content);
    }

    let ext = match format {
        ExportFormat::Csv => "csv",
        ExportFormat::Json => "json",
        ExportFormat::Clipboard => unreachable!(),
    };

    let scope = app.state::<LocalFsScope>();
    let dest: PathBuf = match save_path.filter(|p| !p.is_empty()) {
        Some(path) => {
            let dest = PathBuf::from(path);
            scope.validate_export_path(&dest)?;
            dest
        }
        None => {
            let picked = rfd::FileDialog::new()
                .set_title("Save pack export")
                .set_file_name(format!("{bucket}-pack.{ext}"))
                .add_filter(ext.to_uppercase().as_str(), &[ext])
                .save_file()
                .ok_or_else(|| {
                    PakerError::InvalidInput("Export cancelled: no file selected".to_string())
                })?;
            if let Some(parent) = picked.parent() {
                scope.register_picked_folder(parent);
            }
            picked
        }
    };

    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|_| PakerError::PathNotAllowed)?;
    }

    crate::storage::paths::write_private_file(&dest, content.as_bytes()).map_err(into_ipc_error)?;

    let path_str = dest.to_string_lossy().into_owned();
    let _ = cache.log_pack_export(&connection_id, &bucket, Some(&path_str), keys.len());

    Ok(path_str)
}

// ─── Phase 2: Proposal commands ───────────────────────────────────────────────

#[tauri::command]
pub async fn assistant_build_proposal(
    app: AppHandle,
    input: BuildProposalInput,
) -> Result<ActionProposal, PakerError> {
    let cache = app.state::<ObjectCacheManager>();
    let store = app.state::<ProposalStore>();
    let hmac_key = app.state::<HmacKey>();

    match &input {
        BuildProposalInput::DeleteByQuery(del_input) => {
            let index_age_secs = compute_index_age_secs(&cache, &del_input.connection_id, &del_input.bucket);
            delete_by_query::build(del_input, &cache, &store, &hmac_key, index_age_secs).await
        }
        BuildProposalInput::RenamePattern(ren_input) => {
            let index_age_secs = compute_index_age_secs(&cache, &ren_input.connection_id, &ren_input.bucket);
            rename_pattern::build(ren_input, &cache, &store, &hmac_key, index_age_secs).await
        }
        BuildProposalInput::SyncPlan(sync_input) => {
            sync_plan::build(sync_input, &cache, &store, &hmac_key).await
        }
    }
}

#[tauri::command]
pub async fn assistant_execute_proposal(
    app: AppHandle,
    proposal_id: String,
    token: String,
) -> Result<ExecutionResult, PakerError> {
    let vault = app.state::<VaultManager>();
    vault.ensure_unlocked()?;

    let store = app.state::<ProposalStore>();
    let hmac_key = app.state::<HmacKey>();
    let cache = app.state::<ObjectCacheManager>();
    let audit_log = app.state::<AuditLog>();

    let entry = store.claim(&proposal_id, &token)?;

    let kind_str = entry.kind.to_string();
    hmac_token::verify(
        &hmac_key,
        &token,
        &entry.id,
        &entry.connection_id,
        &entry.bucket,
        &kind_str,
        entry.created_at.timestamp(),
    )?;

    use crate::assistant::builders::ActionKind;
    let result: ExecutionResult = match entry.kind {
        ActionKind::DeleteByQuery => {
            delete_by_query::execute(&entry, &app, &cache).await?
        }
        ActionKind::RenamePattern => rename_pattern::execute(&entry, &app).await?,
        ActionKind::SyncPlan => {
            use tauri::Emitter;
            let _ = app.emit(
                "proposal://progress",
                serde_json::json!({
                    "proposalId": &proposal_id,
                    "done": 0,
                    "total": 0,
                    "phase": "complete",
                }),
            );
            ExecutionResult {
                proposal_id: proposal_id.clone(),
                kind: ActionKind::SyncPlan,
                objects_affected: 0,
                bytes_affected: 0,
                errors: vec![],
            }
        }
    };

    let audit_entry = AuditEntry::new(
        result.proposal_id.clone(),
        result.kind,
        AuditOutcome::Executed,
        entry.connection_id.clone(),
        entry.bucket.clone(),
        result.objects_affected,
        result.bytes_affected,
        result.errors.clone(),
    );
    if let Err(e) = audit_log.append(&audit_entry) {
        tracing::warn!(error = %e, "failed to write audit log entry");
    }

    Ok(result)
}

#[tauri::command]
pub async fn assistant_reject_proposal(
    app: AppHandle,
    proposal_id: String,
    token: String,
) -> Result<(), PakerError> {
    let store = app.state::<ProposalStore>();
    let hmac_key = app.state::<HmacKey>();
    let audit_log = app.state::<AuditLog>();

    let entry = store.reject(&proposal_id)?;

    // Best-effort verify — token expiry allowed on rejection
    let kind_str = entry.kind.to_string();
    let _ = hmac_token::verify(
        &hmac_key,
        &token,
        &entry.id,
        &entry.connection_id,
        &entry.bucket,
        &kind_str,
        entry.created_at.timestamp(),
    );

    let audit_entry = AuditEntry::new(
        proposal_id,
        entry.kind,
        AuditOutcome::Rejected,
        entry.connection_id,
        entry.bucket,
        0,
        0,
        vec![],
    );
    if let Err(e) = audit_log.append(&audit_entry) {
        tracing::warn!(error = %e, "failed to write audit log entry for rejection");
    }

    Ok(())
}

#[tauri::command]
pub async fn assistant_list_proposals(
    app: AppHandle,
    connection_id: Option<String>,
) -> Result<Vec<ProposalEntry>, PakerError> {
    let store = app.state::<ProposalStore>();
    Ok(store.list(connection_id.as_deref()))
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn compute_index_age_secs(
    cache: &ObjectCacheManager,
    connection_id: &str,
    bucket: &str,
) -> Option<u64> {
    let meta = cache.get_bucket_index_meta(connection_id, bucket);
    meta.as_ref().and_then(|m| {
        m.completed_at.as_deref().and_then(|ts| {
            chrono::DateTime::parse_from_rfc3339(ts).ok().map(|dt| {
                (chrono::Utc::now() - dt.with_timezone(&chrono::Utc))
                    .num_seconds()
                    .max(0) as u64
            })
        })
    })
}
