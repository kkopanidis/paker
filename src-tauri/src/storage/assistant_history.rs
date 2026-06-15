use super::object_cache::ObjectCacheManager;
use anyhow::{Context, Result};
use rusqlite::params;
use serde::{Deserialize, Serialize};

const HISTORY_CAP: u32 = 50;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryHistoryItem {
    pub id: i64,
    pub raw_text: String,
    pub summary: String,
    pub confidence: String,
    pub result_count: u64,
    pub created_at: String,
}

impl ObjectCacheManager {
    pub fn insert_query_history(
        &self,
        connection_id: &str,
        bucket: &str,
        raw_text: &str,
        summary: &str,
        confidence: &str,
        result_count: usize,
    ) -> Result<()> {
        {
            let conn = self.db();
            conn.execute(
                "INSERT INTO assistant_query_history
                    (connection_id, bucket, raw_text, summary, confidence, result_count)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    connection_id,
                    bucket,
                    raw_text,
                    summary,
                    confidence,
                    result_count as i64,
                ],
            )
            .context("failed to insert query history")?;
        }
        self.prune_query_history(connection_id, bucket)
    }

    pub fn list_query_history(
        &self,
        connection_id: &str,
        bucket: &str,
        limit: u32,
    ) -> Result<Vec<QueryHistoryItem>> {
        let conn = self.db();
        let mut stmt = conn.prepare(
            "SELECT id, raw_text, summary, confidence, result_count, created_at
             FROM assistant_query_history
             WHERE connection_id = ?1 AND bucket = ?2
             ORDER BY id DESC
             LIMIT ?3",
        )?;

        let rows = stmt.query_map(params![connection_id, bucket, limit], |row| {
            Ok(QueryHistoryItem {
                id: row.get(0)?,
                raw_text: row.get(1)?,
                summary: row.get(2)?,
                confidence: row.get(3)?,
                result_count: row.get::<_, i64>(4)? as u64,
                created_at: row.get(5)?,
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .context("failed to read query history")
    }

    pub fn clear_query_history(&self, connection_id: &str, bucket: &str) -> Result<()> {
        let conn = self.db();
        conn.execute(
            "DELETE FROM assistant_query_history
             WHERE connection_id = ?1 AND bucket = ?2",
            params![connection_id, bucket],
        )
        .context("failed to clear query history")?;
        Ok(())
    }

    pub fn log_pack_export(
        &self,
        connection_id: &str,
        bucket: &str,
        export_path: Option<&str>,
        object_count: usize,
    ) -> Result<()> {
        let conn = self.db();
        conn.execute(
            "INSERT INTO assistant_pack_exports
                (connection_id, bucket, export_path, object_count)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                connection_id,
                bucket,
                export_path,
                object_count as i64,
            ],
        )
        .context("failed to log pack export")?;
        Ok(())
    }

    fn prune_query_history(&self, connection_id: &str, bucket: &str) -> Result<()> {
        let conn = self.db();
        conn.execute(
            "DELETE FROM assistant_query_history
             WHERE connection_id = ?1 AND bucket = ?2
               AND id NOT IN (
                   SELECT id FROM assistant_query_history
                   WHERE connection_id = ?1 AND bucket = ?2
                   ORDER BY id DESC
                   LIMIT ?3
               )",
            params![connection_id, bucket, HISTORY_CAP],
        )
        .context("failed to prune query history")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::object_cache::ObjectCacheManager;
    use std::fs;
    use uuid::Uuid;

    fn open_test_cache() -> ObjectCacheManager {
        let dir =
            std::env::temp_dir().join(format!("paker-assistant-history-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("temp dir");
        ObjectCacheManager::open(dir.join("index.db")).expect("open db")
    }

    #[test]
    fn insert_and_list() {
        let cache = open_test_cache();
        for i in 0..3u32 {
            cache
                .insert_query_history(
                    "conn-1",
                    "bucket",
                    &format!("query {i}"),
                    &format!("summary {i}"),
                    "high",
                    i as usize * 10,
                )
                .expect("insert");
        }

        let items = cache
            .list_query_history("conn-1", "bucket", 20)
            .expect("list");
        assert_eq!(items.len(), 3);
        // newest-first
        assert_eq!(items[0].raw_text, "query 2");
        assert_eq!(items[2].raw_text, "query 0");
    }

    #[test]
    fn prune_at_50() {
        let cache = open_test_cache();
        for i in 0..55u32 {
            cache
                .insert_query_history(
                    "conn-1",
                    "bucket",
                    &format!("q{i}"),
                    "summary",
                    "medium",
                    0,
                )
                .expect("insert");
        }

        let items = cache
            .list_query_history("conn-1", "bucket", 100)
            .expect("list");
        assert!(
            items.len() <= 50,
            "expected ≤50 rows, got {}",
            items.len()
        );
    }

    #[test]
    fn clear_removes_all() {
        let cache = open_test_cache();
        for i in 0..5u32 {
            cache
                .insert_query_history("conn-1", "bucket", &format!("q{i}"), "s", "low", 0)
                .expect("insert");
        }
        cache
            .clear_query_history("conn-1", "bucket")
            .expect("clear");

        let items = cache
            .list_query_history("conn-1", "bucket", 100)
            .expect("list");
        assert_eq!(items.len(), 0);
    }
}
