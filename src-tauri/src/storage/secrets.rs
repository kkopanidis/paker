use super::paths;
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use anyhow::{anyhow, Context, Result};
use argon2::{Algorithm, Argon2, Params, Version};
use keyring_core::{Entry, Error as KeyringError};
use rand::RngCore;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use std::fs;
use tauri::AppHandle;

const PORTABLE_KEY_MATERIAL: &str = "paker-portable-v1";
const NONCE_LEN: usize = 12;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct ConnectionSecrets {
    secret_access_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    session_token: Option<String>,
}

impl<'de> Deserialize<'de> for ConnectionSecrets {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Helper {
            Plain(String),
            Structured {
                secret_access_key: String,
                #[serde(default)]
                session_token: Option<String>,
            },
        }

        match Helper::deserialize(deserializer)? {
            Helper::Plain(secret_access_key) => Ok(ConnectionSecrets {
                secret_access_key,
                session_token: None,
            }),
            Helper::Structured {
                secret_access_key,
                session_token,
            } => Ok(ConnectionSecrets {
                secret_access_key,
                session_token,
            }),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Default, PartialEq, Eq)]
struct SecretsFile {
    secrets: HashMap<String, ConnectionSecrets>,
}

fn derive_portable_key() -> Result<[u8; 32]> {
    let salt = b"paker-portable-salt-v1";
    let params = Params::new(19_456, 2, 1, Some(32))
        .map_err(|e| anyhow!("invalid argon2 params: {e}"))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = [0u8; 32];
    argon2
        .hash_password_into(PORTABLE_KEY_MATERIAL.as_bytes(), salt, &mut key)
        .map_err(|e| anyhow!("argon2 key derivation failed: {e}"))?;
    Ok(key)
}

fn encrypt_secrets(secrets: &SecretsFile) -> Result<Vec<u8>> {
    let key = derive_portable_key()?;
    let cipher = Aes256Gcm::new_from_slice(&key).context("invalid AES key")?;
    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let plaintext =
        serde_json::to_vec(secrets).context("failed to serialize secrets for encryption")?;
    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_ref())
        .map_err(|e| anyhow!("encryption failed: {e}"))?;

    let mut output = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    output.extend_from_slice(&nonce_bytes);
    output.extend(ciphertext);
    Ok(output)
}

fn decrypt_secrets(data: &[u8]) -> Result<SecretsFile> {
    if data.len() <= NONCE_LEN {
        return Err(anyhow!("encrypted secrets file is too short"));
    }

    let key = derive_portable_key()?;
    let cipher = Aes256Gcm::new_from_slice(&key).context("invalid AES key")?;
    let (nonce_bytes, ciphertext) = data.split_at(NONCE_LEN);
    let nonce = Nonce::from_slice(nonce_bytes);

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| anyhow!("decryption failed: {e}"))?;
    serde_json::from_slice(&plaintext).context("failed to parse decrypted secrets")
}

fn read_file_secrets(app: &AppHandle) -> Result<SecretsFile> {
    let path = paths::secrets_path(app)?;
    if !path.exists() {
        return Ok(SecretsFile::default());
    }

    let data = fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
    decrypt_secrets(&data)
}

fn write_file_secrets(app: &AppHandle, secrets: &SecretsFile) -> Result<()> {
    let path = paths::secrets_path(app)?;
    paths::ensure_parent(&path)?;
    let data = encrypt_secrets(secrets)?;
    fs::write(&path, data).with_context(|| format!("failed to write {}", path.display()))
}

fn remove_file_secrets(app: &AppHandle) -> Result<()> {
    let path = paths::secrets_path(app)?;
    if path.exists() {
        fs::remove_file(&path)
            .with_context(|| format!("failed to remove legacy secrets file {}", path.display()))?;
    }
    Ok(())
}

fn keyring_entry(connection_id: &str) -> Result<Entry> {
    Entry::new("paker", connection_id).map_err(|e| anyhow!("failed to create keyring entry: {e}"))
}

fn parse_keyring_value(value: &str) -> ConnectionSecrets {
    if let Ok(secrets) = serde_json::from_str::<ConnectionSecrets>(value) {
        return secrets;
    }
    ConnectionSecrets {
        secret_access_key: value.to_string(),
        session_token: None,
    }
}

fn serialize_keyring_value(secrets: &ConnectionSecrets) -> String {
    if secrets.session_token.is_some() {
        serde_json::to_string(secrets).unwrap_or_else(|_| secrets.secret_access_key.clone())
    } else {
        secrets.secret_access_key.clone()
    }
}

fn read_keyring_secrets(connection_id: &str) -> Result<Option<ConnectionSecrets>> {
    match keyring_entry(connection_id)?.get_password() {
        Ok(value) => Ok(Some(parse_keyring_value(&value))),
        Err(KeyringError::NoEntry) => Ok(None),
        Err(e) => Err(anyhow!("keyring read failed: {e}")),
    }
}

fn write_keyring_secrets(connection_id: &str, secrets: &ConnectionSecrets) -> Result<()> {
    keyring_entry(connection_id)?
        .set_password(&serialize_keyring_value(secrets))
        .context("keyring write failed")
}

fn delete_keyring_secret(connection_id: &str) -> Result<()> {
    match keyring_entry(connection_id)?.delete_credential() {
        Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
        Err(e) => Err(anyhow!("keyring delete failed: {e}")),
    }
}

fn use_file_storage_only() -> bool {
    paths::is_portable_mode()
}

fn get_connection_secrets(app: &AppHandle, connection_id: &str) -> Result<Option<ConnectionSecrets>> {
    if use_file_storage_only() {
        return Ok(read_file_secrets(app)?
            .secrets
            .get(connection_id)
            .cloned());
    }

    if let Some(secrets) = read_keyring_secrets(connection_id)? {
        return Ok(Some(secrets));
    }

    Ok(read_file_secrets(app)?
        .secrets
        .get(connection_id)
        .cloned())
}

pub fn get_secret(app: &AppHandle, connection_id: &str) -> Result<Option<String>> {
    Ok(get_connection_secrets(app, connection_id)?
        .map(|secrets| secrets.secret_access_key))
}

pub fn get_session_token(app: &AppHandle, connection_id: &str) -> Result<Option<String>> {
    Ok(get_connection_secrets(app, connection_id)?
        .and_then(|secrets| secrets.session_token))
}

pub fn set_secrets(
    app: &AppHandle,
    connection_id: &str,
    secret_access_key: &str,
    session_token: Option<&str>,
) -> Result<()> {
    let entry = ConnectionSecrets {
        secret_access_key: secret_access_key.to_string(),
        session_token: session_token.map(|s| s.to_string()),
    };

    if use_file_storage_only() {
        let mut secrets = read_file_secrets(app)?;
        secrets
            .secrets
            .insert(connection_id.to_string(), entry);
        write_file_secrets(app, &secrets)?;
        return Ok(());
    }

    write_keyring_secrets(connection_id, &entry)
}

pub fn delete_secret(app: &AppHandle, connection_id: &str) -> Result<()> {
    if use_file_storage_only() {
        let mut secrets = read_file_secrets(app)?;
        if secrets.secrets.remove(connection_id).is_some() {
            if secrets.secrets.is_empty() {
                remove_file_secrets(app)?;
            } else {
                write_file_secrets(app, &secrets)?;
            }
        }
        return Ok(());
    }

    delete_keyring_secret(connection_id)?;

    let mut secrets = read_file_secrets(app)?;
    if secrets.secrets.remove(connection_id).is_some() {
        if secrets.secrets.is_empty() {
            remove_file_secrets(app)?;
        } else {
            write_file_secrets(app, &secrets)?;
        }
    }

    Ok(())
}

/// One-time migration: move legacy `secrets.enc` entries into the OS keychain (installed mode only).
pub fn migrate_legacy_secrets(app: &AppHandle) -> Result<()> {
    if use_file_storage_only() {
        return Ok(());
    }

    let mut file_secrets = read_file_secrets(app)?;
    if file_secrets.secrets.is_empty() {
        return Ok(());
    }

    let legacy_ids: Vec<String> = file_secrets.secrets.keys().cloned().collect();

    for connection_id in legacy_ids {
        let Some(entry) = file_secrets.secrets.get(&connection_id).cloned() else {
            continue;
        };

        if read_keyring_secrets(&connection_id)?.is_none() {
            write_keyring_secrets(&connection_id, &entry)?;
        }

        file_secrets.secrets.remove(&connection_id);
    }

    if file_secrets.secrets.is_empty() {
        remove_file_secrets(app)?;
    } else {
        write_file_secrets(app, &file_secrets)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portable_encrypt_decrypt_round_trip() {
        let secrets = SecretsFile {
            secrets: HashMap::from([(
                "conn-1".to_string(),
                ConnectionSecrets {
                    secret_access_key: "secret-key".to_string(),
                    session_token: Some("session".to_string()),
                },
            )]),
        };

        let encrypted = encrypt_secrets(&secrets).expect("encrypt");
        let decrypted = decrypt_secrets(&encrypted).expect("decrypt");
        assert_eq!(decrypted, secrets);
    }

    #[test]
    fn portable_encrypt_decrypt_plain_secret_round_trip() {
        let secrets = SecretsFile {
            secrets: HashMap::from([(
                "conn-2".to_string(),
                ConnectionSecrets {
                    secret_access_key: "only-secret".to_string(),
                    session_token: None,
                },
            )]),
        };

        let encrypted = encrypt_secrets(&secrets).expect("encrypt");
        let decrypted = decrypt_secrets(&encrypted).expect("decrypt");
        assert_eq!(decrypted, secrets);
    }

    #[test]
    fn decrypt_rejects_short_ciphertext() {
        let err = decrypt_secrets(&[0u8; NONCE_LEN]).unwrap_err();
        assert!(err.to_string().contains("too short"));
    }

    #[test]
    fn parse_keyring_plain_value() {
        let parsed = parse_keyring_value("my-secret");
        assert_eq!(
            parsed,
            ConnectionSecrets {
                secret_access_key: "my-secret".to_string(),
                session_token: None,
            }
        );
    }

    #[test]
    fn parse_keyring_json_value_with_session_token() {
        let value = r#"{"secret_access_key":"key","session_token":"tok"}"#;
        let parsed = parse_keyring_value(value);
        assert_eq!(
            parsed,
            ConnectionSecrets {
                secret_access_key: "key".to_string(),
                session_token: Some("tok".to_string()),
            }
        );
    }

    #[test]
    fn serialize_keyring_plain_when_no_session_token() {
        let secrets = ConnectionSecrets {
            secret_access_key: "plain".to_string(),
            session_token: None,
        };
        assert_eq!(serialize_keyring_value(&secrets), "plain");
    }

    #[test]
    fn serialize_keyring_json_when_session_token_present() {
        let secrets = ConnectionSecrets {
            secret_access_key: "key".to_string(),
            session_token: Some("tok".to_string()),
        };
        assert_eq!(
            serialize_keyring_value(&secrets),
            r#"{"secret_access_key":"key","session_token":"tok"}"#
        );
    }

    #[test]
    fn use_file_storage_only_respects_portable_env() {
        let previous = std::env::var("PAKER_PORTABLE").ok();
        std::env::set_var("PAKER_PORTABLE", "1");
        assert!(use_file_storage_only());
        match previous {
            Some(value) => std::env::set_var("PAKER_PORTABLE", value),
            None => std::env::remove_var("PAKER_PORTABLE"),
        }
    }
}
