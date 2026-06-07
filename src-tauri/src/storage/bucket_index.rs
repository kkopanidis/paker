use super::object_cache::ObjectCacheManager;
use anyhow::{Context, Result};
use rusqlite::params;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BucketIndexMeta {
    pub connection_id: String,
    pub bucket: String,
    pub status: String,
    pub object_count: u64,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexedObject {
    pub key: String,
    pub size: i64,
    pub last_modified: Option<String>,
    pub etag: Option<String>,
    pub storage_class: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BucketIndexProgress {
    pub connection_id: String,
    pub bucket: String,
    pub object_count: u64,
    pub status: String,
    pub done: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub fn bucket_index_job_id(connection_id: &str, bucket: &str) -> String {
    format!("index:{connection_id}:{bucket}")
}

impl ObjectCacheManager {
    pub fn clear_bucket_index(&self, connection_id: &str, bucket: &str) -> Result<()> {
        let conn = self.db();
        conn.execute(
            "DELETE FROM bucket_index_objects WHERE connection_id = ?1 AND bucket = ?2",
            params![connection_id, bucket],
        )
        .context("failed to clear bucket index objects")?;
        Ok(())
    }

    pub fn get_bucket_index_meta(
        &self,
        connection_id: &str,
        bucket: &str,
    ) -> Option<BucketIndexMeta> {
        let conn = self.db();
        let mut stmt = conn
            .prepare(
                "SELECT status, object_count, started_at, completed_at, error
                 FROM bucket_index_meta
                 WHERE connection_id = ?1 AND bucket = ?2",
            )
            .ok()?;

        stmt.query_row(params![connection_id, bucket], |row| {
            Ok(BucketIndexMeta {
                connection_id: connection_id.to_string(),
                bucket: bucket.to_string(),
                status: row.get(0)?,
                object_count: row.get::<_, i64>(1)? as u64,
                started_at: row.get(2)?,
                completed_at: row.get(3)?,
                error: row.get(4)?,
            })
        })
        .ok()
    }

    pub fn upsert_bucket_index_meta(&self, meta: &BucketIndexMeta) -> Result<()> {
        let conn = self.db();
        conn.execute(
            "INSERT INTO bucket_index_meta
                (connection_id, bucket, status, object_count, started_at, completed_at, error)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(connection_id, bucket) DO UPDATE SET
                status = excluded.status,
                object_count = excluded.object_count,
                started_at = excluded.started_at,
                completed_at = excluded.completed_at,
                error = excluded.error",
            params![
                meta.connection_id,
                meta.bucket,
                meta.status,
                meta.object_count as i64,
                meta.started_at,
                meta.completed_at,
                meta.error,
            ],
        )
        .context("failed to upsert bucket index meta")?;
        Ok(())
    }

    pub fn mark_bucket_index_stale(&self, connection_id: &str, bucket: &str) -> Result<()> {
        let conn = self.db();
        let updated = conn
            .execute(
                "UPDATE bucket_index_meta SET status = 'stale'
                 WHERE connection_id = ?1 AND bucket = ?2 AND status = 'completed'",
                params![connection_id, bucket],
            )
            .context("failed to mark bucket index stale")?;
        if updated == 0 {
            return Ok(());
        }
        Ok(())
    }

    pub fn upsert_indexed_objects_batch(
        &self,
        connection_id: &str,
        bucket: &str,
        objects: &[IndexedObject],
    ) -> Result<()> {
        if objects.is_empty() {
            return Ok(());
        }

        let conn = self.db();
        let tx = conn
            .unchecked_transaction()
            .context("failed to begin bucket index transaction")?;

        for obj in objects {
            tx.execute(
                "INSERT INTO bucket_index_objects
                    (connection_id, bucket, key, size, last_modified, etag, storage_class)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(connection_id, bucket, key) DO UPDATE SET
                    size = excluded.size,
                    last_modified = excluded.last_modified,
                    etag = excluded.etag,
                    storage_class = excluded.storage_class",
                params![
                    connection_id,
                    bucket,
                    obj.key,
                    obj.size,
                    obj.last_modified,
                    obj.etag,
                    obj.storage_class,
                ],
            )
            .context("failed to upsert indexed object")?;
        }

        tx.commit()
            .context("failed to commit bucket index batch")?;
        Ok(())
    }

    pub fn search_bucket_index(
        &self,
        connection_id: &str,
        bucket: &str,
        query: &str,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<IndexedObject>> {
        let pattern = format!("%{}%", query.replace('%', "\\%").replace('_', "\\_"));
        let conn = self.db();
        let mut stmt = conn.prepare(
            "SELECT key, size, last_modified, etag, storage_class
             FROM bucket_index_objects
             WHERE connection_id = ?1 AND bucket = ?2 AND key LIKE ?3 ESCAPE '\\'
             ORDER BY key
             LIMIT ?4 OFFSET ?5",
        )?;

        let rows = stmt.query_map(
            params![connection_id, bucket, pattern, limit, offset],
            |row| {
                Ok(IndexedObject {
                    key: row.get(0)?,
                    size: row.get(1)?,
                    last_modified: row.get(2)?,
                    etag: row.get(3)?,
                    storage_class: row.get(4)?,
                })
            },
        )?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .context("failed to read bucket index search results")
    }

    pub fn export_bucket_index_csv(
        &self,
        connection_id: &str,
        bucket: &str,
    ) -> Result<String> {
        let conn = self.db();
        let mut stmt = conn.prepare(
            "SELECT key, size, last_modified, etag, storage_class
             FROM bucket_index_objects
             WHERE connection_id = ?1 AND bucket = ?2
             ORDER BY key",
        )?;

        let mut csv = String::from("key,size,last_modified,etag,storage_class\n");
        let rows = stmt.query_map(params![connection_id, bucket], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
            ))
        })?;

        for row in rows {
            let (key, size, last_modified, etag, storage_class) =
                row.context("failed to read row for csv export")?;
            csv.push_str(&csv_escape(&key));
            csv.push(',');
            csv.push_str(&size.to_string());
            csv.push(',');
            csv.push_str(&csv_escape(&last_modified.unwrap_or_default()));
            csv.push(',');
            csv.push_str(&csv_escape(&etag.unwrap_or_default()));
            csv.push(',');
            csv.push_str(&csv_escape(&storage_class.unwrap_or_default()));
            csv.push('\n');
        }

        Ok(csv)
    }
}

fn csv_escape(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::object_cache::ObjectCacheManager;
    use std::fs;
    use uuid::Uuid;

    fn temp_db_path() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("paker-bucket-index-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("failed to create temp dir");
        dir.join("index.db")
    }

    fn open_test_cache() -> ObjectCacheManager {
        ObjectCacheManager::open(temp_db_path()).expect("failed to open test cache")
    }

    #[test]
    fn bucket_index_search_and_export() {
        let cache = open_test_cache();
        let objects = vec![
            IndexedObject {
                key: "photos/cat.jpg".to_string(),
                size: 100,
                last_modified: Some("2024-01-01".to_string()),
                etag: Some("\"abc\"".to_string()),
                storage_class: Some("STANDARD".to_string()),
            },
            IndexedObject {
                key: "docs/readme.txt".to_string(),
                size: 50,
                last_modified: None,
                etag: None,
                storage_class: None,
            },
        ];

        cache
            .upsert_indexed_objects_batch("conn-1", "bucket", &objects)
            .expect("batch insert");

        let hits = cache
            .search_bucket_index("conn-1", "bucket", "cat", 10, 0)
            .expect("search");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].key, "photos/cat.jpg");

        let csv = cache
            .export_bucket_index_csv("conn-1", "bucket")
            .expect("export");
        assert!(csv.contains("photos/cat.jpg"));
        assert!(csv.contains("docs/readme.txt"));
    }

    #[test]
    fn mark_bucket_index_stale_only_when_completed() {
        let cache = open_test_cache();
        cache
            .upsert_bucket_index_meta(&BucketIndexMeta {
                connection_id: "conn-1".to_string(),
                bucket: "bucket".to_string(),
                status: "completed".to_string(),
                object_count: 1,
                started_at: None,
                completed_at: Some("1".to_string()),
                error: None,
            })
            .expect("meta");

        cache
            .mark_bucket_index_stale("conn-1", "bucket")
            .expect("stale");

        let meta = cache
            .get_bucket_index_meta("conn-1", "bucket")
            .expect("meta");
        assert_eq!(meta.status, "stale");
    }
}
