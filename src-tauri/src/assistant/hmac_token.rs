use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use hmac::{Hmac, Mac};
use rand::RngCore;
use sha2::Sha256;

use crate::error::PakerError;

type HmacSha256 = Hmac<Sha256>;

pub struct HmacKey(pub [u8; 32]);

impl HmacKey {
    pub fn generate() -> Self {
        let mut bytes = [0u8; 32];
        rand::rng().fill_bytes(&mut bytes);
        HmacKey(bytes)
    }
}

fn make_message(
    proposal_id: &str,
    connection_id: &str,
    bucket: &str,
    kind: &str,
    created_at: i64,
) -> String {
    format!("{proposal_id}:{connection_id}:{bucket}:{kind}:{created_at}")
}

pub fn sign(
    key: &HmacKey,
    proposal_id: &str,
    connection_id: &str,
    bucket: &str,
    kind: &str,
    created_at: i64,
) -> String {
    let message = make_message(proposal_id, connection_id, bucket, kind, created_at);
    let mut mac =
        HmacSha256::new_from_slice(&key.0).expect("HMAC can take key of any size");
    mac.update(message.as_bytes());
    let result = mac.finalize();
    let sig = URL_SAFE_NO_PAD.encode(result.into_bytes());
    format!("v1.{proposal_id}.{sig}")
}

pub fn verify(
    key: &HmacKey,
    token: &str,
    proposal_id: &str,
    connection_id: &str,
    bucket: &str,
    kind: &str,
    created_at: i64,
) -> Result<(), PakerError> {
    let parts: Vec<&str> = token.splitn(3, '.').collect();
    if parts.len() != 3 || parts[0] != "v1" {
        return Err(PakerError::InvalidInput(
            "Malformed HMAC token".to_string(),
        ));
    }
    if parts[1] != proposal_id {
        return Err(PakerError::InvalidInput(
            "Token proposal ID mismatch".to_string(),
        ));
    }
    let sig_bytes = URL_SAFE_NO_PAD
        .decode(parts[2])
        .map_err(|_| PakerError::InvalidInput("Invalid token signature encoding".to_string()))?;

    let message = make_message(proposal_id, connection_id, bucket, kind, created_at);
    let mut mac =
        HmacSha256::new_from_slice(&key.0).expect("HMAC can take key of any size");
    mac.update(message.as_bytes());
    mac.verify_slice(&sig_bytes)
        .map_err(|_| PakerError::InvalidInput("Token signature verification failed".to_string()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_verify_round_trip() {
        let key = HmacKey::generate();
        let token = sign(&key, "id-1", "conn-1", "my-bucket", "deleteByQuery", 1000);
        assert!(verify(&key, &token, "id-1", "conn-1", "my-bucket", "deleteByQuery", 1000).is_ok());
    }

    #[test]
    fn tampered_token_rejected() {
        let key = HmacKey::generate();
        let token = sign(&key, "id-1", "conn-1", "my-bucket", "deleteByQuery", 1000);
        // Tamper the signature
        let tampered = format!("{token}X");
        assert!(
            verify(&key, &tampered, "id-1", "conn-1", "my-bucket", "deleteByQuery", 1000).is_err()
        );
    }

    #[test]
    fn wrong_message_fields_rejected() {
        let key = HmacKey::generate();
        let token = sign(&key, "id-1", "conn-1", "my-bucket", "deleteByQuery", 1000);
        // Different bucket
        assert!(
            verify(&key, &token, "id-1", "conn-1", "other-bucket", "deleteByQuery", 1000).is_err()
        );
    }

    #[test]
    fn wrong_key_rejected() {
        let key1 = HmacKey::generate();
        let key2 = HmacKey::generate();
        let token = sign(&key1, "id-1", "conn-1", "bucket", "deleteByQuery", 1000);
        assert!(verify(&key2, &token, "id-1", "conn-1", "bucket", "deleteByQuery", 1000).is_err());
    }
}
