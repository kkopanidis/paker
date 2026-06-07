use super::paths;
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use anyhow::{anyhow, Context, Result};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::{engine::general_purpose::STANDARD, Engine};
use keyring_core::{Entry, Error as KeyringError};
use rand::RngCore;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use std::fs;
use tauri::AppHandle;

const PORTABLE_KEY_MATERIAL: &str = "paker-portable-v1";
const PORTABLE_KEYRING_SEED_ENTRY: &str = "portable-file-key";
const PORTABLE_SALT_V1: &[u8] = b"paker-portable-salt-v1";
const PORTABLE_SALT_V2: &[u8] = b"paker-portable-salt-v2";
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

fn portable_keyring_seed_entry() -> Result<Entry> {
    Entry::new("paker", PORTABLE_KEYRING_SEED_ENTRY)
        .map_err(|e| anyhow!("failed to create portable keyring entry: {e}"))
}

fn encode_portable_keyring_seed(seed: &[u8; 32]) -> String {
    STANDARD.encode(seed)
}

fn decode_portable_keyring_seed(encoded: &str) -> Result<[u8; 32]> {
    let bytes = STANDARD
        .decode(encoded.trim())
        .context("invalid portable keyring seed encoding")?;
    if bytes.len() != 32 {
        return Err(anyhow!("portable keyring seed must be 32 bytes"));
    }
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&bytes);
    Ok(seed)
}

/// Returns a per-host 32-byte seed stored in the OS keychain, or `None` when the secret service
/// is unavailable (CI, headless environments).
fn get_or_create_portable_keyring_seed() -> Option<[u8; 32]> {
    let entry = match portable_keyring_seed_entry() {
        Ok(entry) => entry,
        Err(error) => {
            tracing::warn!(
                error = %error,
                "portable keyring seed unavailable; using legacy v1 KDF for secrets.enc"
            );
            return None;
        }
    };

    match entry.get_password() {
        Ok(value) => match decode_portable_keyring_seed(&value) {
            Ok(seed) => Some(seed),
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    "invalid portable keyring seed; using legacy v1 KDF for secrets.enc"
                );
                None
            }
        },
        Err(KeyringError::NoEntry) => {
            let mut seed = [0u8; 32];
            rand::rng().fill_bytes(&mut seed);
            if let Err(error) = entry.set_password(&encode_portable_keyring_seed(&seed)) {
                tracing::warn!(
                    error = %error,
                    "failed to store portable keyring seed; using legacy v1 KDF for secrets.enc"
                );
                return None;
            }
            Some(seed)
        }
        Err(error) => {
            tracing::warn!(
                error = %error,
                "portable keyring seed read failed; using legacy v1 KDF for secrets.enc"
            );
            None
        }
    }
}

fn derive_portable_key_v1() -> Result<[u8; 32]> {
    derive_portable_key_from_material(PORTABLE_KEY_MATERIAL.as_bytes(), PORTABLE_SALT_V1)
}

fn derive_portable_key_v2(seed: &[u8]) -> Result<[u8; 32]> {
    let mut material =
        Vec::with_capacity(seed.len() + PORTABLE_KEY_MATERIAL.len());
    material.extend_from_slice(seed);
    material.extend_from_slice(PORTABLE_KEY_MATERIAL.as_bytes());
    derive_portable_key_from_material(&material, PORTABLE_SALT_V2)
}

fn derive_portable_key_from_material(material: &[u8], salt: &[u8]) -> Result<[u8; 32]> {
    let params = Params::new(19_456, 2, 1, Some(32))
        .map_err(|e| anyhow!("invalid argon2 params: {e}"))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = [0u8; 32];
    argon2
        .hash_password_into(material, salt, &mut key)
        .map_err(|e| anyhow!("argon2 key derivation failed: {e}"))?;
    Ok(key)
}

fn portable_encryption_key() -> Result<[u8; 32]> {
    if let Some(seed) = get_or_create_portable_keyring_seed() {
        derive_portable_key_v2(&seed)
    } else {
        derive_portable_key_v1()
    }
}

fn encrypt_secrets_with_key(secrets: &SecretsFile, key: &[u8; 32]) -> Result<Vec<u8>> {
    let cipher = Aes256Gcm::new_from_slice(key).context("invalid AES key")?;
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

fn decrypt_secrets_with_key(data: &[u8], key: &[u8; 32]) -> Result<SecretsFile> {
    if data.len() <= NONCE_LEN {
        return Err(anyhow!("encrypted secrets file is too short"));
    }

    let cipher = Aes256Gcm::new_from_slice(key).context("invalid AES key")?;
    let (nonce_bytes, ciphertext) = data.split_at(NONCE_LEN);
    let nonce = Nonce::from_slice(nonce_bytes);

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| anyhow!("decryption failed: {e}"))?;
    serde_json::from_slice(&plaintext).context("failed to parse decrypted secrets")
}

fn encrypt_secrets(secrets: &SecretsFile) -> Result<Vec<u8>> {
    if get_or_create_portable_keyring_seed().is_none() {
        tracing::warn!(
            "encrypting secrets.enc with legacy v1 KDF because portable keyring seed is unavailable"
        );
    }
    let key = portable_encryption_key()?;
    encrypt_secrets_with_key(secrets, &key)
}

fn decrypt_secrets_with_keyring_seed(
    data: &[u8],
    keyring_seed: Option<[u8; 32]>,
) -> Result<(SecretsFile, bool)> {
    if data.len() <= NONCE_LEN {
        return Err(anyhow!("encrypted secrets file is too short"));
    }

    let has_keyring_seed = keyring_seed.is_some();
    if let Some(seed) = keyring_seed {
        let v2_key = derive_portable_key_v2(&seed)?;
        if let Ok(secrets) = decrypt_secrets_with_key(data, &v2_key) {
            return Ok((secrets, false));
        }
    }

    let v1_key = derive_portable_key_v1()?;
    let secrets = decrypt_secrets_with_key(data, &v1_key)?;
    Ok((secrets, has_keyring_seed))
}

fn decrypt_secrets(data: &[u8]) -> Result<(SecretsFile, bool)> {
    let keyring_seed = get_or_create_portable_keyring_seed();
    decrypt_secrets_with_keyring_seed(data, keyring_seed)
}

fn read_file_secrets(app: &AppHandle) -> Result<SecretsFile> {
    let path = paths::secrets_path(app)?;
    if !path.exists() {
        return Ok(SecretsFile::default());
    }

    let data = fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let (secrets, needs_reencrypt) = decrypt_secrets(&data)?;
    if needs_reencrypt {
        write_file_secrets(app, &secrets)?;
    }
    Ok(secrets)
}

fn write_file_secrets(app: &AppHandle, secrets: &SecretsFile) -> Result<()> {
    let path = paths::secrets_path(app)?;
    paths::ensure_parent(&path)?;
    let data = encrypt_secrets(secrets)?;
    paths::write_private_file(&path, &data)
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

    fn sample_secrets_with_session() -> SecretsFile {
        SecretsFile {
            secrets: HashMap::from([(
                "conn-1".to_string(),
                ConnectionSecrets {
                    secret_access_key: "secret-key".to_string(),
                    session_token: Some("session".to_string()),
                },
            )]),
        }
    }

    fn sample_secrets_plain() -> SecretsFile {
        SecretsFile {
            secrets: HashMap::from([(
                "conn-2".to_string(),
                ConnectionSecrets {
                    secret_access_key: "only-secret".to_string(),
                    session_token: None,
                },
            )]),
        }
    }

    #[test]
    fn derive_portable_key_v1_is_deterministic() {
        let first = derive_portable_key_v1().expect("derive v1");
        let second = derive_portable_key_v1().expect("derive v1 again");
        assert_eq!(first, second);
    }

    #[test]
    fn derive_portable_key_v2_depends_on_seed() {
        let seed_a = [1u8; 32];
        let seed_b = [2u8; 32];
        let key_a = derive_portable_key_v2(&seed_a).expect("derive v2");
        let key_b = derive_portable_key_v2(&seed_b).expect("derive v2 other seed");
        assert_ne!(key_a, key_b);
        assert_ne!(key_a, derive_portable_key_v1().expect("derive v1"));
    }

    #[test]
    fn portable_v2_encrypt_decrypt_round_trip_with_fixed_seed() {
        let secrets = sample_secrets_with_session();
        let seed = [7u8; 32];
        let key = derive_portable_key_v2(&seed).expect("derive v2 key");
        let encrypted = encrypt_secrets_with_key(&secrets, &key).expect("encrypt");
        let (decrypted, needs_reencrypt) =
            decrypt_secrets_with_keyring_seed(&encrypted, Some(seed)).expect("decrypt");
        assert_eq!(decrypted, secrets);
        assert!(!needs_reencrypt);
    }

    #[test]
    fn legacy_v1_blob_decrypts_without_keyring_seed() {
        let secrets = sample_secrets_plain();
        let v1_key = derive_portable_key_v1().expect("derive v1 key");
        let encrypted = encrypt_secrets_with_key(&secrets, &v1_key).expect("encrypt v1");
        let (decrypted, needs_reencrypt) =
            decrypt_secrets_with_keyring_seed(&encrypted, None).expect("decrypt legacy");
        assert_eq!(decrypted, secrets);
        assert!(!needs_reencrypt);
    }

    #[test]
    fn legacy_v1_blob_marks_reencrypt_when_keyring_seed_present() {
        let secrets = sample_secrets_plain();
        let v1_key = derive_portable_key_v1().expect("derive v1 key");
        let encrypted = encrypt_secrets_with_key(&secrets, &v1_key).expect("encrypt v1");
        let seed = [9u8; 32];
        let (decrypted, needs_reencrypt) =
            decrypt_secrets_with_keyring_seed(&encrypted, Some(seed)).expect("decrypt legacy");
        assert_eq!(decrypted, secrets);
        assert!(needs_reencrypt);
    }

    #[test]
    fn portable_encrypt_decrypt_round_trip() {
        let secrets = sample_secrets_with_session();
        let encrypted = encrypt_secrets(&secrets).expect("encrypt");
        let (decrypted, _) = decrypt_secrets(&encrypted).expect("decrypt");
        assert_eq!(decrypted, secrets);
    }

    #[test]
    fn portable_encrypt_decrypt_plain_secret_round_trip() {
        let secrets = sample_secrets_plain();
        let encrypted = encrypt_secrets(&secrets).expect("encrypt");
        let (decrypted, _) = decrypt_secrets(&encrypted).expect("decrypt");
        assert_eq!(decrypted, secrets);
    }

    #[test]
    fn decode_portable_keyring_seed_accepts_base64() {
        let seed = [3u8; 32];
        let encoded = encode_portable_keyring_seed(&seed);
        assert_eq!(
            decode_portable_keyring_seed(&encoded).expect("decode seed"),
            seed
        );
    }

    #[test]
    fn decrypt_rejects_short_ciphertext() {
        let err = decrypt_secrets_with_keyring_seed(&[0u8; NONCE_LEN], None).unwrap_err();
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
