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

#[derive(Debug, Clone, Serialize)]
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

#[derive(Debug, Serialize, Deserialize, Default)]
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
    if let Some(secrets) = read_file_secrets(app)?
        .secrets
        .get(connection_id)
        .cloned()
    {
        return Ok(Some(secrets));
    }

    if use_file_storage_only() {
        return Ok(None);
    }

    read_keyring_secrets(connection_id)
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

    let mut secrets = read_file_secrets(app)?;
    secrets
        .secrets
        .insert(connection_id.to_string(), entry.clone());
    write_file_secrets(app, &secrets)?;

    if !use_file_storage_only() {
        let _ = write_keyring_secrets(connection_id, &entry);
    }

    Ok(())
}

pub fn delete_secret(app: &AppHandle, connection_id: &str) -> Result<()> {
    if !use_file_storage_only() {
        let _ = delete_keyring_secret(connection_id);
    }

    let mut secrets = read_file_secrets(app)?;
    if secrets.secrets.remove(connection_id).is_some() {
        write_file_secrets(app, &secrets)?;
    }
    Ok(())
}
