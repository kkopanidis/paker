pub mod delete_by_query;
pub mod rename_pattern;
pub mod sync_plan;

use crate::assistant::templates::CliCommandSuggestion;

pub const MAX_PREVIEW_ITEMS: usize = 200;

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ActionKind {
    DeleteByQuery,
    RenamePattern,
    SyncPlan,
}

impl std::fmt::Display for ActionKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActionKind::DeleteByQuery => write!(f, "deleteByQuery"),
            ActionKind::RenamePattern => write!(f, "renamePattern"),
            ActionKind::SyncPlan => write!(f, "syncPlan"),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposalItem {
    pub key: String,
    pub size_bytes: u64,
    pub storage_class: Option<String>,
    pub action_description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionProposal {
    pub id: String,
    pub kind: ActionKind,
    pub connection_id: String,
    pub bucket: String,
    pub preview_items: Vec<ProposalItem>,
    pub total_affected: usize,
    pub total_bytes: u64,
    pub warnings: Vec<String>,
    pub token: String,
    pub expires_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cli_suggestions: Option<Vec<CliCommandSuggestion>>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionResult {
    pub proposal_id: String,
    pub kind: ActionKind,
    pub objects_affected: usize,
    pub bytes_affected: u64,
    pub errors: Vec<PartialError>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PartialError {
    pub key: String,
    pub message: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum BuildProposalInput {
    DeleteByQuery(delete_by_query::DeleteByQueryInput),
    RenamePattern(rename_pattern::RenamePatternInput),
    SyncPlan(sync_plan::SyncPlanInput),
}
