mod common;

use common::{
    build_test_client, get_test_bytes, minio_endpoint, put_test_bytes, require_minio,
    test_connection_profile, unique_key, MINIO_SECRET_KEY, TEST_BUCKET,
};
use paker_lib::test_exports::{
    create_folder, delete_objects_batch, delete_objects_expanded, head_object, list_buckets,
    list_objects_v2, object_exists, rename_object, verify_bucket_access,
};
const IGNORE_REASON: &str =
    "requires MinIO; set PAKER_TEST_S3_ENDPOINT and run with --ignored, or use CI rust-integration job";

async fn test_client() -> aws_sdk_s3::Client {
    let profile = test_connection_profile();
    build_test_client(&profile, MINIO_SECRET_KEY).await
}

#[tokio::test]
#[ignore = "requires MinIO (PAKER_TEST_S3_ENDPOINT); CI runs with --ignored"]
async fn list_buckets_and_objects_with_delimiter() {
    require_minio().await.expect(IGNORE_REASON);
    let client = test_client().await;

    let buckets = list_buckets(&client).await.expect("list_buckets");
    assert!(
        buckets.iter().any(|b| b.name == TEST_BUCKET),
        "expected bucket {TEST_BUCKET} in {:?}, endpoint {}",
        buckets.iter().map(|b| &b.name).collect::<Vec<_>>(),
        minio_endpoint()
    );

    let run_id = uuid::Uuid::new_v4().to_string();
    let prefix = format!("integration-test/{run_id}/");
    let folder_name = "delimiter-folder";
    create_folder(&client, TEST_BUCKET, &prefix, folder_name)
        .await
        .expect("create_folder");

    let folder_prefix = format!("{prefix}{folder_name}/");
    let listing = list_objects_v2(&client, TEST_BUCKET, Some(&prefix), None)
        .await
        .expect("list_objects_v2 with prefix");
    assert!(
        listing.common_prefixes.iter().any(|p| p == &folder_prefix),
        "expected common prefix {folder_prefix}, got {:?}",
        listing.common_prefixes
    );

    let nested = list_objects_v2(&client, TEST_BUCKET, Some(&folder_prefix), None)
        .await
        .expect("list_objects_v2 nested");
    assert!(
        nested.objects.iter().any(|o| o.key == folder_prefix),
        "expected folder marker {folder_prefix}, got {:?}",
        nested.objects.iter().map(|o| &o.key).collect::<Vec<_>>()
    );

    delete_objects_batch(&client, TEST_BUCKET, &[folder_prefix])
        .await
        .expect("cleanup folder marker");
}

#[tokio::test]
#[ignore = "requires MinIO (PAKER_TEST_S3_ENDPOINT); CI runs with --ignored"]
async fn upload_download_and_verify_content() {
    require_minio().await.expect(IGNORE_REASON);
    let client = test_client().await;
    let key = unique_key("round-trip");
    let payload = b"paker integration test payload";

    put_test_bytes(&client, &key, payload).await;

    assert!(
        object_exists(&client, TEST_BUCKET, &key)
            .await
            .expect("object_exists"),
        "object should exist after upload"
    );

    let head = head_object(&client, TEST_BUCKET, &key)
        .await
        .expect("head_object");
    assert_eq!(head.key, key);
    assert_eq!(head.content_length, Some(payload.len() as i64));

    let downloaded = get_test_bytes(&client, &key).await;
    assert_eq!(downloaded, payload);

    delete_objects_batch(&client, TEST_BUCKET, &[key])
        .await
        .expect("cleanup object");
}

#[tokio::test]
#[ignore = "requires MinIO (PAKER_TEST_S3_ENDPOINT); CI runs with --ignored"]
async fn delete_objects_removes_keys() {
    require_minio().await.expect(IGNORE_REASON);
    let client = test_client().await;
    let key = unique_key("delete");

    put_test_bytes(&client, &key, b"delete me").await;

    delete_objects_batch(&client, TEST_BUCKET, &[key.clone()])
        .await
        .expect("delete_objects_batch");

    assert!(
        !object_exists(&client, TEST_BUCKET, &key)
            .await
            .expect("object_exists after delete"),
        "object should be gone after delete"
    );
}

#[tokio::test]
#[ignore = "requires MinIO (PAKER_TEST_S3_ENDPOINT); CI runs with --ignored"]
async fn delete_folder_cascades_to_nested_objects() {
    require_minio().await.expect(IGNORE_REASON);
    let client = test_client().await;
    let run_id = uuid::Uuid::new_v4().to_string();
    let prefix = format!("integration-test/{run_id}/");
    let folder_name = "cascade-folder";
    let folder_prefix = format!("{prefix}{folder_name}/");
    let nested_key = format!("{folder_prefix}sub/nested.txt");

    create_folder(&client, TEST_BUCKET, &prefix, folder_name)
        .await
        .expect("create_folder");
    put_test_bytes(&client, &nested_key, b"nested payload").await;

    let listing = list_objects_v2(&client, TEST_BUCKET, Some(&prefix), None)
        .await
        .expect("list before delete");
    assert!(
        listing.common_prefixes.iter().any(|p| p == &folder_prefix),
        "expected folder prefix {folder_prefix}, got {:?}",
        listing.common_prefixes
    );
    assert!(
        object_exists(&client, TEST_BUCKET, &nested_key)
            .await
            .expect("nested object exists before delete"),
        "nested object should exist before delete"
    );

    let deleted = delete_objects_expanded(&client, TEST_BUCKET, &[folder_prefix.clone()])
        .await
        .expect("delete_objects_expanded");
    assert!(
        deleted >= 2,
        "expected folder marker and nested object to be deleted, got {deleted}"
    );

    let after = list_objects_v2(&client, TEST_BUCKET, Some(&prefix), None)
        .await
        .expect("list after delete");
    assert!(
        !after.common_prefixes.iter().any(|p| p == &folder_prefix),
        "folder prefix should be gone after cascade delete, got {:?}",
        after.common_prefixes
    );
    assert!(
        !object_exists(&client, TEST_BUCKET, &nested_key)
            .await
            .expect("nested object gone"),
        "nested object should be gone after cascade delete"
    );
}

#[tokio::test]
#[ignore = "requires MinIO (PAKER_TEST_S3_ENDPOINT); CI runs with --ignored"]
async fn copy_via_rename_object_duplicates_then_moves() {
    require_minio().await.expect(IGNORE_REASON);
    let client = test_client().await;
    let old_key = unique_key("rename-old");
    let new_key = unique_key("rename-new");

    put_test_bytes(&client, &old_key, b"rename payload").await;

    rename_object(&client, TEST_BUCKET, &old_key, &new_key)
        .await
        .expect("rename_object");

    assert!(
        !object_exists(&client, TEST_BUCKET, &old_key)
            .await
            .expect("old key gone"),
        "old key should not exist after rename"
    );
    assert!(
        object_exists(&client, TEST_BUCKET, &new_key)
            .await
            .expect("new key exists"),
        "new key should exist after rename"
    );

    let bytes = get_test_bytes(&client, &new_key).await;
    assert_eq!(bytes, b"rename payload");

    delete_objects_batch(&client, TEST_BUCKET, &[new_key])
        .await
        .expect("cleanup");
}

#[tokio::test]
#[ignore = "requires MinIO (PAKER_TEST_S3_ENDPOINT); CI runs with --ignored"]
async fn verify_bucket_access_like_test_connection() {
    require_minio().await.expect(IGNORE_REASON);
    let client = test_client().await;

    verify_bucket_access(&client, TEST_BUCKET)
        .await
        .expect("verify_bucket_access");

    list_buckets(&client).await.expect("list_buckets fallback");
}
