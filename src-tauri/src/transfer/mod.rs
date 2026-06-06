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
