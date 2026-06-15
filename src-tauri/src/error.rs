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
    #[error("Vault is locked — enter your master key to continue")]
    VaultLocked,
    #[error("Master key verification failed")]
    VaultAuthFailed,
    #[error("Vault unlock is temporarily blocked — try again later")]
    VaultUnlockBlocked,
    #[error("Policy violation: {0}")]
    PolicyViolation(String),
    #[error("Proposal not found")]
    ProposalNotFound,
    #[error("Proposal already claimed")]
    ProposalAlreadyClaimed,
    #[error("Proposal has expired")]
    ProposalExpired,
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
            Self::VaultLocked => "vaultLocked",
            Self::VaultAuthFailed => "vaultAuthFailed",
            Self::VaultUnlockBlocked => "vaultUnlockBlocked",
            Self::PolicyViolation(_) => "policyViolation",
            Self::ProposalNotFound => "proposalNotFound",
            Self::ProposalAlreadyClaimed => "proposalAlreadyClaimed",
            Self::ProposalExpired => "proposalExpired",
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
            Self::VaultLocked => Some("Unlock the vault from the lock screen."),
            Self::VaultAuthFailed => {
                Some("Check your master key or use OS recovery if you forgot it.")
            }
            Self::VaultUnlockBlocked => Some("Wait for the cooldown period before trying again."),
            Self::PolicyViolation(_) => None,
            Self::ProposalNotFound => Some("The proposal may have expired. Build a new proposal."),
            Self::ProposalAlreadyClaimed => {
                Some("This proposal was already executed or rejected.")
            }
            Self::ProposalExpired => Some("Build a new proposal and approve within 15 minutes."),
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
            map_s3_service_error_message(&service_err.err().to_string())
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

fn map_s3_service_error_message(message: &str) -> PakerError {
    if message.contains("NoSuchBucket") || message.contains("NotFound") {
        PakerError::BucketNotFound
    } else if message.contains("AccessDenied")
        || message.contains("Forbidden")
        || message.contains("InvalidAccessKeyId")
        || message.contains("SignatureDoesNotMatch")
        || message.contains("AllAccessDisabled")
    {
        PakerError::AccessDenied
    } else {
        PakerError::Internal
    }
}

/// Clamp presigned URL expiry to 60 seconds .. 7 days.
pub fn clamp_presign_expiry_secs(expires_secs: u64) -> u64 {
    expires_secs.clamp(60, 604_800)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn all_variants() -> Vec<PakerError> {
        vec![
            PakerError::ConnectionNotFound,
            PakerError::BucketNotFound,
            PakerError::AccessDenied,
            PakerError::Network,
            PakerError::InvalidInput("bad".to_string()),
            PakerError::InvalidEndpoint,
            PakerError::PathNotAllowed,
            PakerError::IndexNotReady,
            PakerError::TransferNotFound,
            PakerError::Internal,
            PakerError::VaultLocked,
            PakerError::VaultAuthFailed,
            PakerError::VaultUnlockBlocked,
            PakerError::PolicyViolation("test".to_string()),
            PakerError::ProposalNotFound,
            PakerError::ProposalAlreadyClaimed,
            PakerError::ProposalExpired,
        ]
    }

    #[test]
    fn code_returns_stable_ipc_codes() {
        let expected = [
            ("connectionNotFound", PakerError::ConnectionNotFound),
            ("bucketNotFound", PakerError::BucketNotFound),
            ("accessDenied", PakerError::AccessDenied),
            ("network", PakerError::Network),
            ("invalidInput", PakerError::InvalidInput("x".into())),
            ("invalidEndpoint", PakerError::InvalidEndpoint),
            ("pathNotAllowed", PakerError::PathNotAllowed),
            ("indexNotReady", PakerError::IndexNotReady),
            ("transferNotFound", PakerError::TransferNotFound),
            ("internal", PakerError::Internal),
            ("vaultLocked", PakerError::VaultLocked),
            ("vaultAuthFailed", PakerError::VaultAuthFailed),
            ("vaultUnlockBlocked", PakerError::VaultUnlockBlocked),
            ("policyViolation", PakerError::PolicyViolation("x".into())),
            ("proposalNotFound", PakerError::ProposalNotFound),
            ("proposalAlreadyClaimed", PakerError::ProposalAlreadyClaimed),
            ("proposalExpired", PakerError::ProposalExpired),
        ];
        for (code, err) in expected {
            assert_eq!(err.code(), code);
        }
    }

    #[test]
    fn user_action_is_none_only_for_selected_variants() {
        assert!(PakerError::InvalidInput("x".into()).user_action().is_none());
        assert!(PakerError::TransferNotFound.user_action().is_none());
        for err in all_variants() {
            if matches!(
                err,
                PakerError::InvalidInput(_)
                    | PakerError::TransferNotFound
                    | PakerError::PolicyViolation(_)
            ) {
                continue;
            }
            assert!(
                err.user_action().is_some(),
                "{err:?} should include user_action"
            );
        }
    }

    #[test]
    fn into_ipc_error_preserves_paker_error() {
        let original = PakerError::BucketNotFound;
        let mapped = into_ipc_error(anyhow::Error::new(original.clone()));
        assert_eq!(mapped.code(), original.code());
        assert_eq!(mapped.to_string(), original.to_string());
    }

    #[test]
    fn into_ipc_error_maps_unknown_errors_to_internal() {
        let mapped = into_ipc_error(anyhow::anyhow!("boom"));
        assert!(matches!(mapped, PakerError::Internal));
    }

    #[test]
    fn serialize_includes_camel_case_user_action() {
        let err = PakerError::AccessDenied;
        let value = serde_json::to_value(&err).expect("serialize");
        assert_eq!(value["code"], "accessDenied");
        assert_eq!(value["userAction"], json!(err.user_action().unwrap()));
        assert!(value["message"].is_string());
    }

    #[test]
    fn validate_endpoint_url_accepts_empty_and_http_https() {
        assert!(validate_endpoint_url("").is_ok());
        assert!(validate_endpoint_url("  ").is_ok());
        assert!(validate_endpoint_url("https://s3.amazonaws.com").is_ok());
        assert!(validate_endpoint_url("http://localhost:9000").is_ok());
    }

    #[test]
    fn validate_endpoint_url_rejects_invalid_urls() {
        for endpoint in ["ftp://example.com", "https://", "http://has space"] {
            let err = validate_endpoint_url(endpoint).expect_err(endpoint);
            assert!(matches!(err, PakerError::InvalidEndpoint));
        }
    }

    #[test]
    fn map_s3_service_error_message_classifies_known_codes() {
        assert!(matches!(
            map_s3_service_error_message("NoSuchBucket: bucket missing"),
            PakerError::BucketNotFound
        ));
        assert!(matches!(
            map_s3_service_error_message("404 NotFound"),
            PakerError::BucketNotFound
        ));
        assert!(matches!(
            map_s3_service_error_message("AccessDenied: denied"),
            PakerError::AccessDenied
        ));
        assert!(matches!(
            map_s3_service_error_message("SignatureDoesNotMatch"),
            PakerError::AccessDenied
        ));
        assert!(matches!(
            map_s3_service_error_message("SlowDown"),
            PakerError::Internal
        ));
    }

    #[test]
    fn clamp_presign_expiry_secs_enforces_bounds() {
        assert_eq!(clamp_presign_expiry_secs(1), 60);
        assert_eq!(clamp_presign_expiry_secs(3600), 3600);
        assert_eq!(clamp_presign_expiry_secs(1_000_000), 604_800);
    }
}
