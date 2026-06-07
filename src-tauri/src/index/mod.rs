use std::collections::HashSet;
use std::sync::Mutex;

#[derive(Default)]
pub struct BucketIndexManager {
    running: Mutex<HashSet<String>>,
}

impl BucketIndexManager {
    pub fn try_start(&self, job_id: &str) -> bool {
        self.running.lock().unwrap().insert(job_id.to_string())
    }

    pub fn finish(&self, job_id: &str) {
        self.running.lock().unwrap().remove(job_id);
    }

    pub fn is_running(&self, job_id: &str) -> bool {
        self.running.lock().unwrap().contains(job_id)
    }
}
