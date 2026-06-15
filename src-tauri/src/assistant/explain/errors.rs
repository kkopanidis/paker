use crate::error::PakerError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ErrorExplanation {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_action: Option<String>,
    pub detail: String,
}

pub fn explain_error_code(code: &str) -> ErrorExplanation {
    let err = paker_error_from_code(code);
    ErrorExplanation {
        code: err.code().to_string(),
        message: err.to_string(),
        user_action: err.user_action().map(str::to_string),
        detail: detail_for(&err),
    }
}

fn paker_error_from_code(code: &str) -> PakerError {
    match code {
        "connectionNotFound" => PakerError::ConnectionNotFound,
        "bucketNotFound" => PakerError::BucketNotFound,
        "accessDenied" => PakerError::AccessDenied,
        "network" => PakerError::Network,
        "invalidInput" => PakerError::InvalidInput(String::new()),
        "invalidEndpoint" => PakerError::InvalidEndpoint,
        "pathNotAllowed" => PakerError::PathNotAllowed,
        "indexNotReady" => PakerError::IndexNotReady,
        "transferNotFound" => PakerError::TransferNotFound,
        "internal" => PakerError::Internal,
        "vaultLocked" => PakerError::VaultLocked,
        "vaultAuthFailed" => PakerError::VaultAuthFailed,
        "vaultUnlockBlocked" => PakerError::VaultUnlockBlocked,
        _ => PakerError::InvalidInput(format!("Unknown error code: {code}")),
    }
}

fn detail_for(err: &PakerError) -> String {
    match err {
        PakerError::ConnectionNotFound => {
            "The selected connection profile was removed or its ID is invalid. Open the Connections panel and pick a saved profile, or create a new one.".into()
        }
        PakerError::BucketNotFound => {
            "S3 returned 404 or the bucket name is wrong for this account/endpoint. Confirm the bucket exists in the AWS console or your provider dashboard, and that the profile region matches.".into()
        }
        PakerError::AccessDenied => {
            "Credentials are valid but IAM policy denies the operation. For listing, ensure s3:ListBucket on the bucket ARN. For objects, ensure s3:GetObject/s3:PutObject on the object prefix. Session tokens expire — re-save the profile if using STS.".into()
        }
        PakerError::Network => {
            "Paker could not reach the endpoint. Check VPN/firewall, DNS for custom endpoints, and that the URL includes https://. MinIO and local endpoints must be reachable from this machine.".into()
        }
        PakerError::InvalidInput(msg) if !msg.is_empty() => msg.clone(),
        PakerError::InvalidInput(_) => {
            "The request had invalid parameters (empty path, bad rename target, etc.).".into()
        }
        PakerError::InvalidEndpoint => {
            "Endpoint URLs must start with http:// or https://. Path-style vs virtual-hosted-style is handled automatically; do not append bucket names to the endpoint field.".into()
        }
        PakerError::PathNotAllowed => {
            "Local paths must stay within allowed directories (home, picked folders). Use the folder picker instead of typing arbitrary system paths.".into()
        }
        PakerError::IndexNotReady => {
            "Bucket indexing is idle, running, or failed. Open Index bucket, wait for completion, or rebuild if the index is stale after many changes.".into()
        }
        PakerError::TransferNotFound => {
            "The transfer ID is no longer in the queue — it may have finished or been cancelled.".into()
        }
        PakerError::Internal => {
            "An unexpected Rust-side failure occurred. Restart Paker. If portable mode, ensure ./data/ is writable.".into()
        }
        PakerError::VaultLocked => {
            "Secrets are encrypted with your vault master key. Unlock from the lock screen before connecting or transferring.".into()
        }
        PakerError::VaultAuthFailed => {
            "The master password did not match vault.meta. Use OS recovery (Touch ID / Windows Hello) if configured, or restore from backup.".into()
        }
        PakerError::VaultUnlockBlocked => {
            "Too many failed unlock attempts triggered a cooldown to slow guessing attacks.".into()
        }
        PakerError::PolicyViolation(msg) if !msg.is_empty() => msg.clone(),
        PakerError::PolicyViolation(_) => "The operation was rejected due to a policy violation.".into(),
        PakerError::ProposalNotFound => {
            "The approval proposal was not found. It may have expired or been cancelled.".into()
        }
        PakerError::ProposalAlreadyClaimed => {
            "This proposal was already executed or rejected by another approver.".into()
        }
        PakerError::ProposalExpired => {
            "The approval window elapsed. Build a new proposal and approve within the required time.".into()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explains_access_denied_with_detail() {
        let exp = explain_error_code("accessDenied");
        assert_eq!(exp.code, "accessDenied");
        assert!(exp.detail.contains("IAM"));
    }
}
