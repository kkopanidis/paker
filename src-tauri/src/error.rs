use aws_sdk_s3::error::SdkError;
use serde::ser::{SerializeStruct, Serializer};
use serde::Serialize;
use std::error::Error as StdError;
use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum PakerError {
    #[error("Connection not found")]
    ConnectionNotFound,
    #[error("Bucket not found or inaccessible")]
    BucketNotFound,
    #[error("Access denied — check credentials and permissions")]
    AccessDenied,
    #[error("Network error — could not reach the storage endpoint")]
    Network,
    #[error("{0}")]
    InvalidInput(String),
    #[error("Invalid endpoint URL — use http:// or https://")]
    InvalidEndpoint,
    #[error("Path is not allowed")]
    PathNotAllowed,
    #[error("Bucket index is not ready")]
    IndexNotReady,
    #[error("Transfer not found")]
    TransferNotFound,
    #[error("An internal error occurred")]
    Internal,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IpcErrorPayload {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_action: Option<String>,
}

impl PakerError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::ConnectionNotFound => "connectionNotFound",
            Self::BucketNotFound => "bucketNotFound",
            Self::AccessDenied => "accessDenied",
            Self::Network => "network",
            Self::InvalidInput(_) => "invalidInput",
            Self::InvalidEndpoint => "invalidEndpoint",
            Self::PathNotAllowed => "pathNotAllowed",
            Self::IndexNotReady => "indexNotReady",
            Self::TransferNotFound => "transferNotFound",
            Self::Internal => "internal",
        }
    }

    pub fn user_action(&self) -> Option<&'static str> {
        match self {
            Self::ConnectionNotFound => Some("Open Connections and add or select a profile."),
            Self::BucketNotFound => {
                Some("Verify the bucket name and that your account can access it.")
            }
            Self::AccessDenied => {
                Some("Check your access key, secret key, session token, and IAM permissions.")
            }
            Self::Network => {
                Some("Check your network connection and that the endpoint is reachable.")
            }
            Self::InvalidInput(_) => None,
            Self::InvalidEndpoint => {
                Some("Enter a full http:// or https:// URL for the S3-compatible endpoint.")
            }
            Self::PathNotAllowed => {
                Some("Choose a file or folder from your home directory or use the file picker.")
            }
            Self::IndexNotReady => {
                Some("Wait for indexing to finish or start a new index for this bucket.")
            }
            Self::TransferNotFound => None,
            Self::Internal => Some("Try again. If the problem persists, restart the app."),
        }
    }

    pub fn to_ipc_payload(&self) -> IpcErrorPayload {
        IpcErrorPayload {
            code: self.code().to_string(),
            message: self.to_string(),
            user_action: self.user_action().map(str::to_string),
        }
    }
}

impl Serialize for PakerError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let payload = self.to_ipc_payload();
        let mut state = serializer.serialize_struct("PakerError", 3)?;
        state.serialize_field("code", &payload.code)?;
        state.serialize_field("message", &payload.message)?;
        if let Some(user_action) = &payload.user_action {
            state.serialize_field("userAction", user_action)?;
        }
        state.end()
    }
}

pub fn validate_endpoint_url(endpoint: &str) -> Result<(), PakerError> {
    let endpoint = endpoint.trim();
    if endpoint.is_empty() {
        return Ok(());
    }

    let rest = endpoint
        .strip_prefix("https://")
        .or_else(|| endpoint.strip_prefix("http://"))
        .ok_or(PakerError::InvalidEndpoint)?;

    if rest.is_empty() || rest.contains(char::is_whitespace) {
        return Err(PakerError::InvalidEndpoint);
    }

    Ok(())
}

pub fn map_s3_sdk_error<E>(err: SdkError<E>) -> PakerError
where
    E: StdError + 'static,
{
    match &err {
        SdkError::TimeoutError(_) | SdkError::DispatchFailure(_) => PakerError::Network,
        SdkError::ConstructionFailure(_) => PakerError::Internal,
        SdkError::ResponseError(_) => PakerError::Network,
        SdkError::ServiceError(service_err) => {
            let code = service_err.err().to_string();
            if code.contains("NoSuchBucket") || code.contains("NotFound") {
                PakerError::BucketNotFound
            } else if code.contains("AccessDenied")
                || code.contains("Forbidden")
                || code.contains("InvalidAccessKeyId")
                || code.contains("SignatureDoesNotMatch")
                || code.contains("AllAccessDisabled")
            {
                PakerError::AccessDenied
            } else {
                PakerError::Internal
            }
        }
        _ => PakerError::Internal,
    }
}

pub fn into_ipc_error(err: impl Into<anyhow::Error>) -> PakerError {
    let err = err.into();
    if let Some(pe) = err.downcast_ref::<PakerError>() {
        return pe.clone();
    }
    tracing::debug!(error = %err, "mapping internal error to IPC");
    PakerError::Internal
}

/// Clamp presigned URL expiry to 60 seconds .. 7 days.
pub fn clamp_presign_expiry_secs(expires_secs: u64) -> u64 {
    expires_secs.clamp(60, 604_800)
}
