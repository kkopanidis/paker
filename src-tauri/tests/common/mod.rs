use paker_lib::test_exports::{build_insecure_http_client, endpoint_uses_https, ConnectionProfile};
use std::time::Duration;

pub const TEST_BUCKET: &str = "paker-test";
pub const MINIO_ACCESS_KEY: &str = "minioadmin";
pub const MINIO_SECRET_KEY: &str = "minioadmin";

/// MinIO / S3-compatible endpoint for integration tests.
pub fn minio_endpoint() -> String {
    std::env::var("PAKER_TEST_S3_ENDPOINT")
        .or_else(|_| std::env::var("MINIO_ENDPOINT"))
        .unwrap_or_else(|_| "http://127.0.0.1:9000".to_string())
}

pub fn test_connection_profile() -> ConnectionProfile {
    ConnectionProfile {
        id: "integration-test-minio".to_string(),
        name: "MinIO Integration Test".to_string(),
        endpoint: Some(minio_endpoint()),
        region: "us-east-1".to_string(),
        access_key_id: MINIO_ACCESS_KEY.to_string(),
        force_path_style: true,
        skip_tls_verify: false,
        default_bucket: Some(TEST_BUCKET.to_string()),
    }
}

/// Returns an error when MinIO is not reachable (for explicit skip messages).
pub async fn require_minio() -> Result<(), String> {
    let endpoint = minio_endpoint();
    let health_url = format!("{}/minio/health/live", endpoint.trim_end_matches('/'));

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .map_err(|e| format!("failed to build HTTP client: {e}"))?;

    client
        .get(&health_url)
        .send()
        .await
        .map_err(|e| {
            format!(
                "MinIO not reachable at {endpoint} ({health_url}): {e}. \
                 Start MinIO locally or set PAKER_TEST_S3_ENDPOINT / MINIO_ENDPOINT."
            )
        })?
        .error_for_status()
        .map_err(|e| format!("MinIO health check failed at {health_url}: {e}"))?;

    Ok(())
}

/// Build an S3 client for integration tests without a Tauri `AppHandle`.
pub async fn build_test_client(
    profile: &ConnectionProfile,
    secret_access_key: &str,
) -> aws_sdk_s3::Client {
    use aws_config::BehaviorVersion;
    use aws_credential_types::Credentials;
    use aws_sdk_s3::config::{Builder as S3ConfigBuilder, Region};

    let credentials = Credentials::new(
        profile.access_key_id.clone(),
        secret_access_key,
        None,
        None,
        "paker-integration-test",
    );

    let mut config_builder = S3ConfigBuilder::new()
        .behavior_version(BehaviorVersion::latest())
        .credentials_provider(credentials)
        .region(Region::new(profile.region.clone()));

    if let Some(endpoint) = &profile.endpoint {
        if !endpoint.is_empty() {
            config_builder = config_builder.endpoint_url(endpoint);
        }
    }

    if profile.force_path_style {
        config_builder = config_builder.force_path_style(true);
    }

    if profile.skip_tls_verify && endpoint_uses_https(profile.endpoint.as_deref()) {
        config_builder = config_builder.http_client(build_insecure_http_client());
    }

    aws_sdk_s3::Client::from_conf(config_builder.build())
}

pub fn unique_key(prefix: &str) -> String {
    format!("integration-test/{prefix}/{}", uuid::Uuid::new_v4())
}

/// Upload bytes via the AWS SDK (setup helper — `put_object_file` needs a live Tauri app).
pub async fn put_test_bytes(client: &aws_sdk_s3::Client, key: &str, payload: &[u8]) {
    use aws_sdk_s3::primitives::ByteStream;

    client
        .put_object()
        .bucket(TEST_BUCKET)
        .key(key)
        .body(ByteStream::from(payload.to_vec()))
        .send()
        .await
        .expect("put_object setup upload");
}

/// Download object bytes via the AWS SDK (assertion helper — `get_object_to_path` needs Tauri).
pub async fn get_test_bytes(client: &aws_sdk_s3::Client, key: &str) -> Vec<u8> {
    let response = client
        .get_object()
        .bucket(TEST_BUCKET)
        .key(key)
        .send()
        .await
        .expect("get_object download");

    response
        .body
        .collect()
        .await
        .expect("read object body")
        .into_bytes()
        .to_vec()
}
