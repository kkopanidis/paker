use crate::assistant::builders::ActionKind;
use crate::error::PakerError;
use parking_lot::Mutex;
use std::collections::HashMap;
use uuid::Uuid;

pub const PROPOSAL_TTL_SECS: u64 = 900;

/// Generate a new UUIDv4 proposal ID.
pub fn new_proposal_id() -> String {
    Uuid::new_v4().to_string()
}

/// Compute expiry timestamp for a proposal created at `now`.
pub fn proposal_expires_at(
    now: chrono::DateTime<chrono::Utc>,
) -> chrono::DateTime<chrono::Utc> {
    now + chrono::Duration::seconds(PROPOSAL_TTL_SECS as i64)
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ProposalStatus {
    Pending,
    Executed,
    Rejected,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposalEntry {
    pub id: String,
    pub kind: ActionKind,
    pub connection_id: String,
    pub bucket: String,
    pub payload: serde_json::Value,
    pub token: String,
    pub status: ProposalStatus,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

pub struct ProposalStore {
    inner: Mutex<HashMap<String, ProposalEntry>>,
}

impl Default for ProposalStore {
    fn default() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
        }
    }
}

impl ProposalStore {
    /// Insert a freshly built proposal; evicts expired entries before inserting.
    pub fn insert(&self, entry: ProposalEntry) -> ProposalEntry {
        let mut map = self.inner.lock();
        let now = chrono::Utc::now();
        map.retain(|_, v| v.expires_at > now);
        map.insert(entry.id.clone(), entry.clone());
        entry
    }

    /// Attempt to claim a pending, non-expired proposal for execution.
    pub fn claim(&self, id: &str, token: &str) -> Result<ProposalEntry, PakerError> {
        let mut map = self.inner.lock();
        let entry = map.get_mut(id).ok_or(PakerError::ProposalNotFound)?;

        if !matches!(entry.status, ProposalStatus::Pending) {
            return Err(PakerError::ProposalAlreadyClaimed);
        }
        if chrono::Utc::now() > entry.expires_at {
            return Err(PakerError::ProposalExpired);
        }
        if entry.token != token {
            return Err(PakerError::InvalidInput("token mismatch".to_string()));
        }

        entry.status = ProposalStatus::Executed;
        Ok(entry.clone())
    }

    /// Mark a proposal as rejected.
    pub fn reject(&self, id: &str) -> Result<ProposalEntry, PakerError> {
        let mut map = self.inner.lock();
        let entry = map.get_mut(id).ok_or(PakerError::ProposalNotFound)?;

        if !matches!(entry.status, ProposalStatus::Pending) {
            return Err(PakerError::ProposalAlreadyClaimed);
        }

        entry.status = ProposalStatus::Rejected;
        Ok(entry.clone())
    }

    /// List proposals, optionally filtered by connection_id.  Returns at most 50, newest first.
    pub fn list(&self, connection_id: Option<&str>) -> Vec<ProposalEntry> {
        let map = self.inner.lock();
        let mut entries: Vec<ProposalEntry> = map
            .values()
            .filter(|e| {
                connection_id
                    .map(|cid| e.connection_id == cid)
                    .unwrap_or(true)
            })
            .cloned()
            .collect();
        entries.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        entries.truncate(50);
        entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant::builders::ActionKind;

    fn make_entry(id: &str, token: &str) -> ProposalEntry {
        let now = chrono::Utc::now();
        ProposalEntry {
            id: id.to_string(),
            kind: ActionKind::DeleteByQuery,
            connection_id: "conn".to_string(),
            bucket: "bucket".to_string(),
            payload: serde_json::Value::Null,
            token: token.to_string(),
            status: ProposalStatus::Pending,
            created_at: now,
            expires_at: proposal_expires_at(now),
        }
    }

    #[test]
    fn insert_and_claim_succeeds() {
        let store = ProposalStore::default();
        let entry = make_entry("id1", "tok1");
        store.insert(entry);
        let claimed = store.claim("id1", "tok1").expect("claim should succeed");
        assert_eq!(claimed.id, "id1");
    }

    #[test]
    fn double_claim_returns_err() {
        let store = ProposalStore::default();
        store.insert(make_entry("id2", "tok2"));
        store.claim("id2", "tok2").expect("first claim ok");
        let err = store.claim("id2", "tok2").expect_err("second claim should fail");
        assert!(matches!(err, PakerError::ProposalAlreadyClaimed));
    }

    #[test]
    fn expired_entry_cannot_be_claimed() {
        let store = ProposalStore::default();
        let now = chrono::Utc::now();
        let entry = ProposalEntry {
            id: "id3".to_string(),
            kind: ActionKind::DeleteByQuery,
            connection_id: "conn".to_string(),
            bucket: "bucket".to_string(),
            payload: serde_json::Value::Null,
            token: "tok3".to_string(),
            status: ProposalStatus::Pending,
            created_at: now - chrono::Duration::seconds(PROPOSAL_TTL_SECS as i64 + 10),
            expires_at: now - chrono::Duration::seconds(10),
        };
        store.insert(entry);
        let err = store.claim("id3", "tok3").expect_err("expired claim should fail");
        assert!(matches!(err, PakerError::ProposalExpired));
    }

    #[test]
    fn wrong_token_returns_invalid_input() {
        let store = ProposalStore::default();
        store.insert(make_entry("id4", "tok4"));
        let err = store.claim("id4", "wrong").expect_err("wrong token should fail");
        assert!(matches!(err, PakerError::InvalidInput(_)));
    }

    #[test]
    fn reject_transitions_status() {
        let store = ProposalStore::default();
        store.insert(make_entry("id5", "tok5"));
        let entry = store.reject("id5").expect("reject should succeed");
        assert!(matches!(entry.status, ProposalStatus::Rejected));
    }
}
