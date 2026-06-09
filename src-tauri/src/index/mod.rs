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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_start_deduplicates_running_jobs() {
        let mgr = BucketIndexManager::default();
        assert!(mgr.try_start("job-1"));
        assert!(!mgr.try_start("job-1"));
        assert!(mgr.is_running("job-1"));
    }

    #[test]
    fn finish_allows_job_to_start_again() {
        let mgr = BucketIndexManager::default();
        assert!(mgr.try_start("job-1"));
        mgr.finish("job-1");
        assert!(!mgr.is_running("job-1"));
        assert!(mgr.try_start("job-1"));
    }

    #[test]
    fn different_job_ids_are_tracked_independently() {
        let mgr = BucketIndexManager::default();
        assert!(mgr.try_start("job-a"));
        assert!(mgr.try_start("job-b"));
        mgr.finish("job-a");
        assert!(!mgr.is_running("job-a"));
        assert!(mgr.is_running("job-b"));
    }
}
