use super::paths;
use crate::s3::{ListObjectsResult, ObjectHeadResult, PrefixSizeResult};
use anyhow::{Context, Result};
use lru::LruCache;
use rusqlite::{params, Connection};
use std::hash::{Hash, Hasher};
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::AppHandle;

const LISTINGS_LRU_CAPACITY: usize = 64;

#[derive(Debug, Clone, Eq)]
struct ListingCacheKey {
    connection_id: String,
    bucket: String,
    prefix: String,
}

impl PartialEq for ListingCacheKey {
    fn eq(&self, other: &Self) -> bool {
        self.connection_id == other.connection_id
            && self.bucket == other.bucket
            && self.prefix == other.prefix
    }
}

impl Hash for ListingCacheKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.connection_id.hash(state);
        self.bucket.hash(state);
        self.prefix.hash(state);
    }
}

#[derive(Debug, Clone)]
struct CachedListing {
    result: ListObjectsResult,
    fetched_at: String,
}

pub struct ObjectCacheManager {
    conn: Mutex<Connection>,
    listings_lru: Mutex<LruCache<ListingCacheKey, CachedListing>>,
}

fn timestamp_now() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}

fn parent_listing_prefix(prefix: &str) -> Option<String> {
    if prefix.is_empty() {
        return None;
    }

    let without_trailing = prefix.strip_suffix('/').unwrap_or(prefix);
    if without_trailing.is_empty() {
        return Some(String::new());
    }

    match without_trailing.rfind('/') {
        None => Some(String::new()),
        Some(0) => Some(String::new()),
        Some(idx) => Some(format!("{}/", &without_trailing[..idx])),
    }
}

fn prefix_like_pattern(prefix: &str) -> String {
    format!("{prefix}%")
}

impl ObjectCacheManager {
    pub(crate) fn db(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap()
    }

    pub fn new(app: &AppHandle) -> Result<Self> {
        let db_path = paths::index_db_path(app)?;
        Self::open(db_path)
    }

    pub fn open(db_path: PathBuf) -> Result<Self> {
        paths::ensure_parent(&db_path)?;
        let conn = Connection::open(&db_path).with_context(|| {
            format!("failed to open object cache database {}", db_path.display())
        })?;
        Self::init_schema(&conn)?;

        let capacity = NonZeroUsize::new(LISTINGS_LRU_CAPACITY).expect("LRU capacity must be > 0");

        Ok(Self {
            conn: Mutex::new(conn),
            listings_lru: Mutex::new(LruCache::new(capacity)),
        })
    }

    fn init_schema(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS listings (
                connection_id TEXT NOT NULL,
                bucket TEXT NOT NULL,
                prefix TEXT NOT NULL,
                continuation_token TEXT NOT NULL,
                fetched_at TEXT NOT NULL,
                json TEXT NOT NULL,
                PRIMARY KEY (connection_id, bucket, prefix, continuation_token)
            );

            CREATE TABLE IF NOT EXISTS head_objects (
                connection_id TEXT NOT NULL,
                bucket TEXT NOT NULL,
                key TEXT NOT NULL,
                fetched_at TEXT NOT NULL,
                json TEXT NOT NULL,
                PRIMARY KEY (connection_id, bucket, key)
            );

            CREATE TABLE IF NOT EXISTS prefix_sizes (
                connection_id TEXT NOT NULL,
                bucket TEXT NOT NULL,
                prefix TEXT NOT NULL,
                object_count INTEGER NOT NULL,
                total_bytes INTEGER NOT NULL,
                calculated_at TEXT NOT NULL,
                PRIMARY KEY (connection_id, bucket, prefix)
            );

            CREATE TABLE IF NOT EXISTS bucket_index_objects (
                connection_id TEXT NOT NULL,
                bucket TEXT NOT NULL,
                key TEXT NOT NULL,
                size INTEGER NOT NULL,
                last_modified TEXT,
                etag TEXT,
                storage_class TEXT,
                PRIMARY KEY (connection_id, bucket, key)
            );

            CREATE INDEX IF NOT EXISTS idx_bucket_index_key
                ON bucket_index_objects(connection_id, bucket, key);

            CREATE TABLE IF NOT EXISTS bucket_index_meta (
                connection_id TEXT NOT NULL,
                bucket TEXT NOT NULL,
                status TEXT NOT NULL,
                object_count INTEGER NOT NULL DEFAULT 0,
                started_at TEXT,
                completed_at TEXT,
                error TEXT,
                PRIMARY KEY (connection_id, bucket)
            );

            CREATE TABLE IF NOT EXISTS assistant_query_history (
                id           INTEGER PRIMARY KEY AUTOINCREMENT,
                connection_id TEXT    NOT NULL,
                bucket        TEXT    NOT NULL,
                raw_text      TEXT    NOT NULL,
                summary       TEXT    NOT NULL,
                confidence    TEXT    NOT NULL CHECK(confidence IN ('high','medium','low')),
                result_count  INTEGER NOT NULL DEFAULT 0,
                created_at    TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
            );

            CREATE INDEX IF NOT EXISTS idx_aqh_conn_bucket
                ON assistant_query_history(connection_id, bucket, id DESC);

            CREATE TABLE IF NOT EXISTS assistant_pack_exports (
                id            INTEGER PRIMARY KEY AUTOINCREMENT,
                connection_id TEXT NOT NULL,
                bucket        TEXT NOT NULL,
                export_path   TEXT,
                object_count  INTEGER NOT NULL,
                created_at    TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
            );
            ",
        )
        .context("failed to initialize object cache schema")?;

        Ok(())
    }

    pub fn get_listing(
        &self,
        connection_id: &str,
        bucket: &str,
        prefix: &str,
        continuation_token: &str,
    ) -> Option<(ListObjectsResult, String)> {
        if continuation_token.is_empty() {
            let key = ListingCacheKey {
                connection_id: connection_id.to_string(),
                bucket: bucket.to_string(),
                prefix: prefix.to_string(),
            };

            if let Some(cached) = self.listings_lru.lock().unwrap().get(&key) {
                return Some((cached.result.clone(), cached.fetched_at.clone()));
            }
        }

        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT json, fetched_at FROM listings
                 WHERE connection_id = ?1 AND bucket = ?2 AND prefix = ?3 AND continuation_token = ?4",
            )
            .ok()?;

        let result = stmt
            .query_row(
                params![connection_id, bucket, prefix, continuation_token],
                |row| {
                    let json: String = row.get(0)?;
                    let fetched_at: String = row.get(1)?;
                    Ok((json, fetched_at))
                },
            )
            .ok()?;

        let listing: ListObjectsResult = serde_json::from_str(&result.0).ok()?;

        if continuation_token.is_empty() {
            let key = ListingCacheKey {
                connection_id: connection_id.to_string(),
                bucket: bucket.to_string(),
                prefix: prefix.to_string(),
            };
            self.listings_lru.lock().unwrap().put(
                key,
                CachedListing {
                    result: listing.clone(),
                    fetched_at: result.1.clone(),
                },
            );
        }

        Some((listing, result.1))
    }

    pub fn put_listing(
        &self,
        connection_id: &str,
        bucket: &str,
        prefix: &str,
        continuation_token: &str,
        result: &ListObjectsResult,
    ) -> Result<()> {
        let fetched_at = timestamp_now();
        let json = serde_json::to_string(result)
            .context("failed to serialize listing for object cache")?;

        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO listings (connection_id, bucket, prefix, continuation_token, fetched_at, json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(connection_id, bucket, prefix, continuation_token)
             DO UPDATE SET fetched_at = excluded.fetched_at, json = excluded.json",
            params![
                connection_id,
                bucket,
                prefix,
                continuation_token,
                fetched_at,
                json
            ],
        )
        .context("failed to write listing to object cache")?;

        if continuation_token.is_empty() {
            let key = ListingCacheKey {
                connection_id: connection_id.to_string(),
                bucket: bucket.to_string(),
                prefix: prefix.to_string(),
            };
            self.listings_lru.lock().unwrap().put(
                key,
                CachedListing {
                    result: result.clone(),
                    fetched_at,
                },
            );
        }

        Ok(())
    }

    pub fn invalidate_prefix(&self, connection_id: &str, bucket: &str, prefix: &str) -> Result<()> {
        self.invalidate_listings_lru_under_prefix(connection_id, bucket, prefix);

        let like_pattern = prefix_like_pattern(prefix);
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "DELETE FROM listings
             WHERE connection_id = ?1 AND bucket = ?2 AND (prefix = ?3 OR prefix LIKE ?4)",
            params![connection_id, bucket, prefix, like_pattern],
        )
        .context("failed to invalidate cached listings")?;

        conn.execute(
            "DELETE FROM head_objects
             WHERE connection_id = ?1 AND bucket = ?2 AND key LIKE ?3",
            params![connection_id, bucket, like_pattern],
        )
        .context("failed to invalidate cached head objects under prefix")?;

        conn.execute(
            "DELETE FROM prefix_sizes
             WHERE connection_id = ?1 AND bucket = ?2 AND (prefix = ?3 OR prefix LIKE ?4)",
            params![connection_id, bucket, prefix, like_pattern],
        )
        .context("failed to invalidate cached prefix sizes")?;

        Ok(())
    }

    pub fn invalidate_parent_if_needed(
        &self,
        connection_id: &str,
        bucket: &str,
        prefix: &str,
    ) -> Result<()> {
        let Some(parent) = parent_listing_prefix(prefix) else {
            return Ok(());
        };

        self.invalidate_listings_lru_exact(connection_id, bucket, &parent);

        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM listings
             WHERE connection_id = ?1 AND bucket = ?2 AND prefix = ?3",
            params![connection_id, bucket, parent],
        )
        .context("failed to invalidate parent listing in object cache")?;

        Ok(())
    }

    pub fn get_head(
        &self,
        connection_id: &str,
        bucket: &str,
        key: &str,
    ) -> Option<(ObjectHeadResult, String)> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT json, fetched_at FROM head_objects
                 WHERE connection_id = ?1 AND bucket = ?2 AND key = ?3",
            )
            .ok()?;

        let (json, fetched_at) = stmt
            .query_row(params![connection_id, bucket, key], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .ok()?;

        let head: ObjectHeadResult = serde_json::from_str(&json).ok()?;
        Some((head, fetched_at))
    }

    pub fn put_head(
        &self,
        connection_id: &str,
        bucket: &str,
        key: &str,
        result: &ObjectHeadResult,
    ) -> Result<()> {
        let fetched_at = timestamp_now();
        let json =
            serde_json::to_string(result).context("failed to serialize head object for cache")?;

        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO head_objects (connection_id, bucket, key, fetched_at, json)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(connection_id, bucket, key)
             DO UPDATE SET fetched_at = excluded.fetched_at, json = excluded.json",
            params![connection_id, bucket, key, fetched_at, json],
        )
        .context("failed to write head object to cache")?;

        Ok(())
    }

    pub fn invalidate_head(&self, connection_id: &str, bucket: &str, key: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM head_objects WHERE connection_id = ?1 AND bucket = ?2 AND key = ?3",
            params![connection_id, bucket, key],
        )
        .context("failed to invalidate cached head object")?;

        Ok(())
    }

    pub fn get_prefix_size(
        &self,
        connection_id: &str,
        bucket: &str,
        prefix: &str,
    ) -> Option<(PrefixSizeResult, String)> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT prefix, object_count, total_bytes, calculated_at FROM prefix_sizes
                 WHERE connection_id = ?1 AND bucket = ?2 AND prefix = ?3",
            )
            .ok()?;

        stmt.query_row(params![connection_id, bucket, prefix], |row| {
            Ok((
                PrefixSizeResult {
                    prefix: row.get(0)?,
                    object_count: row.get::<_, i64>(1)? as u64,
                    total_bytes: row.get::<_, i64>(2)? as u64,
                },
                row.get::<_, String>(3)?,
            ))
        })
        .ok()
    }

    pub fn put_prefix_size(
        &self,
        connection_id: &str,
        bucket: &str,
        prefix: &str,
        result: &PrefixSizeResult,
    ) -> Result<()> {
        let calculated_at = timestamp_now();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO prefix_sizes (connection_id, bucket, prefix, object_count, total_bytes, calculated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(connection_id, bucket, prefix)
             DO UPDATE SET
                object_count = excluded.object_count,
                total_bytes = excluded.total_bytes,
                calculated_at = excluded.calculated_at",
            params![
                connection_id,
                bucket,
                prefix,
                result.object_count as i64,
                result.total_bytes as i64,
                calculated_at
            ],
        )
        .context("failed to write prefix size to cache")?;

        Ok(())
    }

    fn invalidate_listings_lru_exact(&self, connection_id: &str, bucket: &str, prefix: &str) {
        let key = ListingCacheKey {
            connection_id: connection_id.to_string(),
            bucket: bucket.to_string(),
            prefix: prefix.to_string(),
        };
        self.listings_lru.lock().unwrap().pop(&key);
    }

    fn invalidate_listings_lru_under_prefix(
        &self,
        connection_id: &str,
        bucket: &str,
        prefix: &str,
    ) {
        let mut lru = self.listings_lru.lock().unwrap();
        let keys: Vec<ListingCacheKey> = lru
            .iter()
            .filter_map(|(key, _)| {
                if key.connection_id == connection_id
                    && key.bucket == bucket
                    && (key.prefix == prefix || key.prefix.starts_with(prefix))
                {
                    Some(key.clone())
                } else {
                    None
                }
            })
            .collect();

        for key in keys {
            lru.pop(&key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::s3::operations::ObjectInfo;
    use std::collections::HashMap;
    use std::fs;
    use uuid::Uuid;

    fn sample_listing() -> ListObjectsResult {
        ListObjectsResult {
            objects: vec![ObjectInfo {
                key: "photos/cat.jpg".to_string(),
                size: 1024,
                last_modified: None,
                etag: None,
                storage_class: None,
                is_prefix: false,
            }],
            common_prefixes: vec!["photos/dogs/".to_string()],
            continuation_token: None,
            is_truncated: false,
            prefix_last_modified: HashMap::new(),
        }
    }

    fn temp_db_path() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("paker-object-cache-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("failed to create temp dir");
        dir.join("index.db")
    }

    fn open_test_cache() -> ObjectCacheManager {
        let db_path = temp_db_path();
        ObjectCacheManager::open(db_path).expect("failed to open test cache")
    }

    #[test]
    fn first_page_listing_uses_lru() {
        let cache = open_test_cache();
        let listing = sample_listing();

        cache
            .put_listing("conn-1", "my-bucket", "photos/", "", &listing)
            .expect("put listing");

        let (from_lru, _) = cache
            .get_listing("conn-1", "my-bucket", "photos/", "")
            .expect("listing should be cached");

        assert_eq!(from_lru.objects.len(), 1);
        assert_eq!(from_lru.common_prefixes, vec!["photos/dogs/".to_string()]);
    }

    #[test]
    fn continuation_token_bypasses_lru() {
        let cache = open_test_cache();
        let first_page = sample_listing();
        let second_page = ListObjectsResult {
            objects: vec![ObjectInfo {
                key: "photos/zebra.jpg".to_string(),
                size: 2048,
                last_modified: None,
                etag: None,
                storage_class: None,
                is_prefix: false,
            }],
            common_prefixes: vec![],
            continuation_token: Some("token-2".to_string()),
            is_truncated: false,
            prefix_last_modified: HashMap::new(),
        };

        cache
            .put_listing("conn-1", "bucket", "photos/", "", &first_page)
            .expect("put first page");
        cache
            .put_listing("conn-1", "bucket", "photos/", "token-2", &second_page)
            .expect("put second page");

        let fetched = cache
            .get_listing("conn-1", "bucket", "photos/", "token-2")
            .expect("second page should be in sqlite");
        assert_eq!(fetched.0.objects[0].key, "photos/zebra.jpg");

        cache.listings_lru.lock().unwrap().clear();
        let still_there = cache
            .get_listing("conn-1", "bucket", "photos/", "token-2")
            .expect("sqlite should back second page");
        assert_eq!(still_there.0.objects[0].size, 2048);
    }

    #[test]
    fn invalidate_prefix_removes_listings_heads_and_sizes() {
        let cache = open_test_cache();
        let listing = sample_listing();
        let head = ObjectHeadResult {
            key: "photos/cat.jpg".to_string(),
            content_type: Some("image/jpeg".to_string()),
            content_length: Some(1024),
            last_modified: None,
            etag: None,
            metadata: Default::default(),
            storage_class: None,
        };
        let size = PrefixSizeResult {
            prefix: "photos/".to_string(),
            object_count: 1,
            total_bytes: 1024,
        };

        cache
            .put_listing("conn-1", "bucket", "photos/", "", &listing)
            .expect("put listing");
        cache
            .put_listing("conn-1", "bucket", "photos/nested/", "", &listing)
            .expect("put nested listing");
        cache
            .put_head("conn-1", "bucket", "photos/cat.jpg", &head)
            .expect("put head");
        cache
            .put_prefix_size("conn-1", "bucket", "photos/", &size)
            .expect("put prefix size");

        cache
            .invalidate_prefix("conn-1", "bucket", "photos/")
            .expect("invalidate prefix");

        assert!(cache
            .get_listing("conn-1", "bucket", "photos/", "")
            .is_none());
        assert!(cache
            .get_listing("conn-1", "bucket", "photos/nested/", "")
            .is_none());
        assert!(cache
            .get_head("conn-1", "bucket", "photos/cat.jpg")
            .is_none());
        assert!(cache
            .get_prefix_size("conn-1", "bucket", "photos/")
            .is_none());
    }

    #[test]
    fn invalidate_parent_if_needed_clears_parent_listing_only() {
        let cache = open_test_cache();
        let parent_listing = ListObjectsResult {
            objects: vec![],
            common_prefixes: vec!["photos/".to_string()],
            continuation_token: None,
            is_truncated: false,
            prefix_last_modified: HashMap::new(),
        };
        let child_listing = sample_listing();
        let child_head = ObjectHeadResult {
            key: "photos/cat.jpg".to_string(),
            content_type: None,
            content_length: None,
            last_modified: None,
            etag: None,
            metadata: Default::default(),
            storage_class: None,
        };

        cache
            .put_listing("conn-1", "bucket", "", "", &parent_listing)
            .expect("put parent listing");
        cache
            .put_listing("conn-1", "bucket", "photos/", "", &child_listing)
            .expect("put child listing");
        cache
            .put_head("conn-1", "bucket", "photos/cat.jpg", &child_head)
            .expect("put child head");

        cache
            .invalidate_parent_if_needed("conn-1", "bucket", "photos/")
            .expect("invalidate parent");

        assert!(cache.get_listing("conn-1", "bucket", "", "").is_none());
        assert!(cache
            .get_listing("conn-1", "bucket", "photos/", "")
            .is_some());
        assert!(cache
            .get_head("conn-1", "bucket", "photos/cat.jpg")
            .is_some());
    }

    #[test]
    fn lru_evicts_oldest_entry_at_capacity() {
        let db_path = temp_db_path();
        let capacity = NonZeroUsize::new(2).unwrap();
        let conn = Connection::open(&db_path).expect("open db");
        ObjectCacheManager::init_schema(&conn).expect("init schema");

        let cache = ObjectCacheManager {
            conn: Mutex::new(conn),
            listings_lru: Mutex::new(LruCache::new(capacity)),
        };

        let listing = sample_listing();
        cache
            .put_listing("conn", "bucket", "a/", "", &listing)
            .expect("put a");
        cache
            .put_listing("conn", "bucket", "b/", "", &listing)
            .expect("put b");
        cache
            .put_listing("conn", "bucket", "c/", "", &listing)
            .expect("put c");

        let key_a = ListingCacheKey {
            connection_id: "conn".to_string(),
            bucket: "bucket".to_string(),
            prefix: "a/".to_string(),
        };
        let key_b = ListingCacheKey {
            connection_id: "conn".to_string(),
            bucket: "bucket".to_string(),
            prefix: "b/".to_string(),
        };
        let key_c = ListingCacheKey {
            connection_id: "conn".to_string(),
            bucket: "bucket".to_string(),
            prefix: "c/".to_string(),
        };

        let lru = cache.listings_lru.lock().unwrap();
        assert!(lru.peek(&key_a).is_none());
        assert!(lru.peek(&key_b).is_some());
        assert!(lru.peek(&key_c).is_some());
    }
}
