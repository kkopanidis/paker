use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use tokio_util::sync::CancellationToken;

#[derive(Default)]
pub struct TransferManager {
    tokens: Mutex<HashMap<String, CancellationToken>>,
    paused: Mutex<HashSet<String>>,
}

impl TransferManager {
    pub fn register(&self, transfer_id: &str) -> CancellationToken {
        let token = CancellationToken::new();
        self.tokens
            .lock()
            .unwrap()
            .insert(transfer_id.to_string(), token.clone());
        token
    }

    pub fn cancel(&self, transfer_id: &str) -> bool {
        let tokens = self.tokens.lock().unwrap();
        if let Some(token) = tokens.get(transfer_id) {
            token.cancel();
            true
        } else {
            false
        }
    }

    pub fn pause(&self, transfer_id: &str) -> bool {
        self.paused.lock().unwrap().insert(transfer_id.to_string())
    }

    pub fn resume(&self, transfer_id: &str) -> bool {
        self.paused.lock().unwrap().remove(transfer_id)
    }

    pub fn is_paused(&self, transfer_id: &str) -> bool {
        self.paused.lock().unwrap().contains(transfer_id)
    }

    pub fn remove(&self, transfer_id: &str) {
        self.tokens.lock().unwrap().remove(transfer_id);
        self.paused.lock().unwrap().remove(transfer_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_creates_token_and_cancel_cancels_registered_transfer() {
        let mgr = TransferManager::default();
        let token = mgr.register("transfer-1");
        assert!(!token.is_cancelled());
        assert!(mgr.cancel("transfer-1"));
        assert!(token.is_cancelled());
    }

    #[test]
    fn pause_resume_is_paused_lifecycle() {
        let mgr = TransferManager::default();
        mgr.register("transfer-1");

        assert!(!mgr.is_paused("transfer-1"));
        assert!(mgr.pause("transfer-1"));
        assert!(mgr.is_paused("transfer-1"));
        assert!(!mgr.pause("transfer-1"));
        assert!(mgr.resume("transfer-1"));
        assert!(!mgr.is_paused("transfer-1"));
        assert!(!mgr.resume("transfer-1"));
    }

    #[test]
    fn remove_cleans_up_tokens_and_paused_set() {
        let mgr = TransferManager::default();
        mgr.register("transfer-1");
        mgr.pause("transfer-1");

        mgr.remove("transfer-1");

        assert!(!mgr.cancel("transfer-1"));
        assert!(!mgr.is_paused("transfer-1"));
    }

    #[test]
    fn cancel_on_unknown_transfer_returns_false() {
        let mgr = TransferManager::default();
        assert!(!mgr.cancel("missing"));
    }
}
