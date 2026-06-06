use crate::storage::{get_secret, get_session_token, ConnectionProfile};
use anyhow::{anyhow, Result};
use aws_config::BehaviorVersion;
use aws_credential_types::Credentials;
use aws_sdk_s3::config::{Builder as S3ConfigBuilder, Region};
use aws_sdk_s3::Client;
use tauri::AppHandle;

pub async fn build_client(app: &AppHandle, profile: &ConnectionProfile) -> Result<Client> {
    let secret = get_secret(app, &profile.id)?.ok_or_else(|| {
        anyhow!(
            "no secret stored for connection {} — edit the connection and re-enter the secret access key",
            profile.id
        )
    })?;

    let session_token = get_session_token(app, &profile.id)?;

    let credentials = Credentials::new(
        profile.access_key_id.clone(),
        secret,
        session_token,
        None,
        "paker",
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

    Ok(Client::from_conf(config_builder.build()))
}

pub async fn build_client_for_id(app: &AppHandle, connection_id: &str) -> Result<Client> {
    let profile = crate::storage::get_connection(app, connection_id)?
        .ok_or_else(|| anyhow!("connection not found: {connection_id}"))?;
    build_client(app, &profile).await
}
