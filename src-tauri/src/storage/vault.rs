use super::paths;
use super::secrets::{
    collect_legacy_secrets, decrypt_secrets_with_key, delete_all_keyring_secrets,
    encrypt_secrets_with_key, SecretsFile,
};
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use anyhow::{anyhow, Context, Result};
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::{engine::general_purpose::STANDARD, Engine};
use keyring_core::{Entry, Error as KeyringError};
use parking_lot::Mutex;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::fs;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use zeroize::{Zeroize, Zeroizing};

const VAULT_META_VERSION: u32 = 1;
const VAULT_RECOVERY_ENTRY: &str = "vault-recovery";
const NONCE_LEN: usize = 12;
const MIN_MASTER_PASSWORD_LEN: usize = 8;
const MAX_UNLOCK_ATTEMPTS_BEFORE_BACKOFF: u32 = 5;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultStatus {
    pub enabled: bool,
    pub locked: bool,
    pub setup_required: bool,
    pub auto_lock_minutes: u32,
    pub lock_on_blur: bool,
    pub recovery_available: bool,
    pub unlock_blocked_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VaultMeta {
    version: u32,
    enabled: bool,
    verifier_salt: String,
    verifier_hash: String,
    wrapped_vault_key: String,
    auto_lock_minutes: u32,
    #[serde(default)]
    lock_on_blur: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct VaultRecoveryBlob {
    escrow_key: String,
    wrapped_vault_key: String,
}

struct VaultRuntime {
    meta: Option<VaultMeta>,
    locked: bool,
    vault_key: Option<Zeroizing<[u8; 32]>>,
    last_activity: Instant,
    failed_unlock_attempts: u32,
    unlock_blocked_until: Option<Instant>,
}

impl Default for VaultRuntime {
    fn default() -> Self {
        Self {
            meta: None,
            locked: false,
            vault_key: None,
            last_activity: Instant::now(),
            failed_unlock_attempts: 0,
            unlock_blocked_until: None,
        }
    }
}

pub struct VaultManager {
    inner: Mutex<VaultRuntime>,
}

impl Default for VaultManager {
    fn default() -> Self {
        Self {
            inner: Mutex::new(VaultRuntime::default()),
        }
    }
}

impl VaultManager {
    pub fn load_from_disk(&self, app: &AppHandle) -> Result<()> {
        let mut state = self.inner.lock();
        let path = paths::vault_meta_path(app)?;
        if !path.exists() {
            state.meta = None;
            state.locked = false;
            state.vault_key = None;
            return Ok(());
        }

        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let meta: VaultMeta = serde_json::from_str(&raw).context("failed to parse vault.meta")?;
        if !meta.enabled {
            state.meta = Some(meta);
            state.locked = false;
            state.vault_key = None;
            return Ok(());
        }

        state.meta = Some(meta);
        state.locked = true;
        state.vault_key = None;
        state.last_activity = Instant::now();
        Ok(())
    }

    pub fn is_enabled(&self) -> bool {
        self.inner.lock().meta.as_ref().is_some_and(|m| m.enabled)
    }

    pub fn is_locked(&self) -> bool {
        let state = self.inner.lock();
        state.meta.as_ref().is_some_and(|m| m.enabled) && state.locked
    }

    pub fn ensure_unlocked(&self) -> Result<(), crate::error::PakerError> {
        if self.is_enabled() && self.is_locked() {
            return Err(crate::error::PakerError::VaultLocked);
        }
        Ok(())
    }

    pub fn status(&self, app: &AppHandle) -> Result<VaultStatus> {
        let state = self.inner.lock();
        let enabled = state.meta.as_ref().is_some_and(|m| m.enabled);
        let locked = enabled && state.locked;
        let (auto_lock_minutes, lock_on_blur) = state
            .meta
            .as_ref()
            .map(|m| (m.auto_lock_minutes, m.lock_on_blur))
            .unwrap_or((15, false));

        let unlock_blocked_secs = state
            .unlock_blocked_until
            .and_then(|until| {
                until
                    .checked_duration_since(Instant::now())
                    .map(|d| d.as_secs())
            })
            .unwrap_or(0);

        Ok(VaultStatus {
            enabled,
            locked,
            setup_required: !paths::vault_meta_path(app)?.exists(),
            auto_lock_minutes,
            lock_on_blur,
            recovery_available: recovery_entry_available(),
            unlock_blocked_secs,
        })
    }

    pub fn record_activity(&self) {
        let mut state = self.inner.lock();
        state.last_activity = Instant::now();
    }

    pub fn idle_lock_elapsed(&self, auto_lock_minutes: u32) -> bool {
        if auto_lock_minutes == 0 {
            return false;
        }
        let state = self.inner.lock();
        state.last_activity.elapsed() >= Duration::from_secs(u64::from(auto_lock_minutes) * 60)
    }

    pub fn lock(&self, app: &AppHandle) -> Result<()> {
        let mut state = self.inner.lock();
        if !state.meta.as_ref().is_some_and(|m| m.enabled) {
            return Ok(());
        }
        if let Some(mut key) = state.vault_key.take() {
            key.zeroize();
        }
        state.locked = true;
        drop(state);
        let _ = app.emit("vault-locked", ());
        purge_preview_cache(app)?;
        tracing::info!("vault locked");
        Ok(())
    }

    pub fn setup(
        &self,
        app: &AppHandle,
        master_password: &str,
        auto_lock_minutes: u32,
        lock_on_blur: bool,
    ) -> Result<()> {
        validate_master_password(master_password)?;

        let legacy = collect_legacy_secrets(app)?;
        let mut vault_key = Zeroizing::new([0u8; 32]);
        rand::rng().fill_bytes(vault_key.as_mut());

        let verifier_salt = SaltString::generate(&mut argon2_os_rng());
        let verifier_hash = hash_master_password(master_password, &verifier_salt)?;
        let wrapped_vault_key = wrap_vault_key(
            &vault_key,
            master_password,
            verifier_salt.as_salt().as_str(),
        )?;

        write_vault_secrets_file(app, &legacy, &vault_key)?;
        delete_all_keyring_secrets(app, &legacy)?;

        let meta = VaultMeta {
            version: VAULT_META_VERSION,
            enabled: true,
            verifier_salt: verifier_salt.to_string(),
            verifier_hash,
            wrapped_vault_key,
            auto_lock_minutes,
            lock_on_blur,
        };
        write_vault_meta(app, &meta)?;
        if let Err(error) = store_recovery_escrow(&vault_key) {
            tracing::warn!(
                error = %error,
                "recovery escrow unavailable; OS reset will not work on this system"
            );
        }

        let mut state = self.inner.lock();
        state.meta = Some(meta);
        state.locked = false;
        state.vault_key = Some(vault_key);
        state.last_activity = Instant::now();
        state.failed_unlock_attempts = 0;
        state.unlock_blocked_until = None;
        let _ = app.emit("vault-unlocked", ());
        tracing::info!("vault enabled");
        Ok(())
    }

    pub fn unlock(
        &self,
        app: &AppHandle,
        master_password: &str,
    ) -> Result<(), crate::error::PakerError> {
        if let Some(until) = self.inner.lock().unlock_blocked_until {
            if Instant::now() < until {
                return Err(crate::error::PakerError::VaultUnlockBlocked);
            }
        }

        let meta = self
            .inner
            .lock()
            .meta
            .clone()
            .filter(|m| m.enabled)
            .ok_or(crate::error::PakerError::VaultAuthFailed)?;

        let verified =
            verify_master_password(master_password, &meta.verifier_salt, &meta.verifier_hash)
                .unwrap_or(false);
        if !verified {
            self.register_failed_unlock();
            return Err(crate::error::PakerError::VaultAuthFailed);
        }

        let vault_key = unwrap_vault_key(
            &meta.wrapped_vault_key,
            master_password,
            &meta.verifier_salt,
        )
        .map_err(|_| crate::error::PakerError::VaultAuthFailed)?;

        let mut state = self.inner.lock();
        state.vault_key = Some(vault_key);
        state.locked = false;
        state.last_activity = Instant::now();
        state.failed_unlock_attempts = 0;
        state.unlock_blocked_until = None;
        drop(state);
        let _ = app.emit("vault-unlocked", ());
        tracing::info!("vault unlocked");
        Ok(())
    }

    pub fn change_master_key(
        &self,
        app: &AppHandle,
        current_password: &str,
        new_password: &str,
    ) -> Result<(), crate::error::PakerError> {
        self.ensure_unlocked()?;
        validate_master_password(new_password)
            .map_err(|e| crate::error::PakerError::InvalidInput(e.to_string()))?;

        let vault_key_bytes = {
            let state = self.inner.lock();
            let meta = state
                .meta
                .as_ref()
                .filter(|m| m.enabled)
                .ok_or(crate::error::PakerError::VaultAuthFailed)?;

            if !verify_master_password(current_password, &meta.verifier_salt, &meta.verifier_hash)
                .map_err(|_| crate::error::PakerError::VaultAuthFailed)?
            {
                return Err(crate::error::PakerError::VaultAuthFailed);
            }

            let vault_key = state
                .vault_key
                .as_ref()
                .ok_or(crate::error::PakerError::VaultLocked)?;
            **vault_key
        };

        let verifier_salt = SaltString::generate(&mut argon2_os_rng());
        let verifier_hash = hash_master_password(new_password, &verifier_salt)
            .map_err(|_| crate::error::PakerError::Internal)?;
        let wrapped_vault_key = wrap_vault_key(
            &vault_key_bytes,
            new_password,
            verifier_salt.as_salt().as_str(),
        )
        .map_err(|_| crate::error::PakerError::Internal)?;

        let meta_clone = {
            let mut state = self.inner.lock();
            let meta = state
                .meta
                .as_mut()
                .filter(|m| m.enabled)
                .ok_or(crate::error::PakerError::VaultAuthFailed)?;
            meta.verifier_salt = verifier_salt.to_string();
            meta.verifier_hash = verifier_hash;
            meta.wrapped_vault_key = wrapped_vault_key;
            meta.clone()
        };
        write_vault_meta(app, &meta_clone).map_err(|_| crate::error::PakerError::Internal)?;
        store_recovery_escrow(&vault_key_bytes).map_err(|_| crate::error::PakerError::Internal)?;
        tracing::info!("vault master key changed");
        Ok(())
    }

    pub fn reset_master_key_with_os_auth(
        &self,
        app: &AppHandle,
        new_password: &str,
    ) -> Result<(), crate::error::PakerError> {
        if !recovery_entry_available() {
            return Err(crate::error::PakerError::InvalidInput(
                "OS recovery is not available on this system".to_string(),
            ));
        }

        crate::os_auth::verify_os_user("Authenticate to reset your Paker master key")
            .map_err(|_| crate::error::PakerError::VaultAuthFailed)?;

        validate_master_password(new_password)
            .map_err(|e| crate::error::PakerError::InvalidInput(e.to_string()))?;

        let vault_key = load_recovery_escrow().map_err(|_| crate::error::PakerError::Internal)?;

        let verifier_salt = SaltString::generate(&mut argon2_os_rng());
        let verifier_hash = hash_master_password(new_password, &verifier_salt)
            .map_err(|_| crate::error::PakerError::Internal)?;
        let wrapped_vault_key =
            wrap_vault_key(&vault_key, new_password, verifier_salt.as_salt().as_str())
                .map_err(|_| crate::error::PakerError::Internal)?;

        let mut state = self.inner.lock();
        let meta = state
            .meta
            .as_mut()
            .filter(|m| m.enabled)
            .ok_or(crate::error::PakerError::VaultAuthFailed)?;

        meta.verifier_salt = verifier_salt.to_string();
        meta.verifier_hash = verifier_hash;
        meta.wrapped_vault_key = wrapped_vault_key;
        let meta_clone = meta.clone();
        state.vault_key = Some(vault_key);
        state.locked = false;
        state.last_activity = Instant::now();
        state.failed_unlock_attempts = 0;
        state.unlock_blocked_until = None;
        drop(state);

        write_vault_meta(app, &meta_clone).map_err(|_| crate::error::PakerError::Internal)?;
        tracing::info!("vault master key reset via OS auth");
        Ok(())
    }

    pub fn set_preferences(
        &self,
        app: &AppHandle,
        auto_lock_minutes: u32,
        lock_on_blur: bool,
    ) -> Result<(), crate::error::PakerError> {
        let mut state = self.inner.lock();
        let meta = state.meta.as_mut().filter(|m| m.enabled).ok_or(
            crate::error::PakerError::InvalidInput("Vault is not enabled".to_string()),
        )?;
        meta.auto_lock_minutes = auto_lock_minutes;
        meta.lock_on_blur = lock_on_blur;
        let meta_clone = meta.clone();
        drop(state);
        write_vault_meta(app, &meta_clone).map_err(|_| crate::error::PakerError::Internal)?;
        Ok(())
    }

    fn register_failed_unlock(&self) {
        let mut state = self.inner.lock();
        state.failed_unlock_attempts += 1;
        if state.failed_unlock_attempts >= MAX_UNLOCK_ATTEMPTS_BEFORE_BACKOFF {
            let exponent = state
                .failed_unlock_attempts
                .saturating_sub(MAX_UNLOCK_ATTEMPTS_BEFORE_BACKOFF);
            let secs = 30u64.saturating_mul(2u64.saturating_pow(exponent.min(4)));
            state.unlock_blocked_until = Some(Instant::now() + Duration::from_secs(secs.min(300)));
        }
    }
}

pub fn read_vault_secrets(app: &AppHandle) -> Result<SecretsFile> {
    let vault = app.state::<VaultManager>();
    let vault_key = vault
        .inner
        .lock()
        .vault_key
        .clone()
        .ok_or_else(|| anyhow!("vault is locked"))?;

    let path = paths::secrets_path(app)?;
    if !path.exists() {
        return Ok(SecretsFile::default());
    }

    let data = fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
    decrypt_secrets_with_key(&data, &vault_key).context("failed to decrypt vault secrets")
}

pub fn write_vault_secrets(app: &AppHandle, secrets: &SecretsFile) -> Result<()> {
    let vault = app.state::<VaultManager>();
    let vault_key = vault
        .inner
        .lock()
        .vault_key
        .clone()
        .ok_or_else(|| anyhow!("vault is locked"))?;
    write_vault_secrets_file(app, secrets, &vault_key)
}

fn write_vault_secrets_file(
    app: &AppHandle,
    secrets: &SecretsFile,
    vault_key: &[u8; 32],
) -> Result<()> {
    let path = paths::secrets_path(app)?;
    paths::ensure_parent(&path)?;
    let data = encrypt_secrets_with_key(secrets, vault_key)?;
    if secrets.secrets.is_empty() {
        if path.exists() {
            fs::remove_file(&path)
                .with_context(|| format!("failed to remove {}", path.display()))?;
        }
        return Ok(());
    }
    paths::write_private_file(&path, &data)
}

fn write_vault_meta(app: &AppHandle, meta: &VaultMeta) -> Result<()> {
    let path = paths::vault_meta_path(app)?;
    paths::ensure_parent(&path)?;
    let json = serde_json::to_vec_pretty(meta).context("failed to serialize vault.meta")?;
    paths::write_private_file(&path, &json)
}

fn validate_master_password(password: &str) -> Result<()> {
    if password.len() < MIN_MASTER_PASSWORD_LEN {
        return Err(anyhow!(
            "master key must be at least {MIN_MASTER_PASSWORD_LEN} characters"
        ));
    }
    Ok(())
}

fn argon2_instance() -> Result<Argon2<'static>> {
    let params =
        Params::new(19_456, 2, 1, Some(32)).map_err(|e| anyhow!("invalid argon2 params: {e}"))?;
    Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
}

fn argon2_os_rng() -> argon2::password_hash::rand_core::OsRng {
    argon2::password_hash::rand_core::OsRng
}

fn hash_master_password(password: &str, salt: &SaltString) -> Result<String> {
    let argon2 = argon2_instance()?;
    let hash = argon2
        .hash_password(password.as_bytes(), salt)
        .map_err(|e| anyhow!("failed to hash master password: {e}"))?;
    Ok(hash.to_string())
}

fn verify_master_password(password: &str, _salt_b64: &str, hash_b64: &str) -> Result<bool> {
    let argon2 = argon2_instance()?;
    let parsed = PasswordHash::new(hash_b64).map_err(|e| anyhow!("invalid hash: {e}"))?;
    Ok(argon2.verify_password(password.as_bytes(), &parsed).is_ok())
}

fn derive_wrap_key(password: &str, salt: &str) -> Result<[u8; 32]> {
    let argon2 = argon2_instance()?;
    let mut key = [0u8; 32];
    argon2
        .hash_password_into(
            password.as_bytes(),
            format!("paker-vault-key-wrap:{salt}").as_bytes(),
            &mut key,
        )
        .map_err(|e| anyhow!("wrap key derivation failed: {e}"))?;
    Ok(key)
}

fn wrap_vault_key(vault_key: &[u8; 32], password: &str, salt: &str) -> Result<String> {
    let wrap_key = derive_wrap_key(password, salt)?;
    let cipher = Aes256Gcm::new_from_slice(&wrap_key).context("invalid wrap key")?;
    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, vault_key.as_ref())
        .map_err(|e| anyhow!("vault key wrap failed: {e}"))?;
    let mut output = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    output.extend_from_slice(&nonce_bytes);
    output.extend(ciphertext);
    Ok(STANDARD.encode(output))
}

fn unwrap_vault_key(wrapped_b64: &str, password: &str, salt: &str) -> Result<Zeroizing<[u8; 32]>> {
    let data = STANDARD
        .decode(wrapped_b64.trim())
        .context("invalid wrapped vault key encoding")?;
    if data.len() <= NONCE_LEN {
        return Err(anyhow!("wrapped vault key is too short"));
    }
    let wrap_key = derive_wrap_key(password, salt)?;
    let cipher = Aes256Gcm::new_from_slice(&wrap_key).context("invalid wrap key")?;
    let (nonce_bytes, ciphertext) = data.split_at(NONCE_LEN);
    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| anyhow!("vault key unwrap failed: {e}"))?;
    if plaintext.len() != 32 {
        return Err(anyhow!("unexpected vault key length"));
    }
    let mut key = Zeroizing::new([0u8; 32]);
    key.copy_from_slice(&plaintext);
    Ok(key)
}

fn recovery_entry() -> Result<Entry> {
    Entry::new("paker", VAULT_RECOVERY_ENTRY)
        .map_err(|e| anyhow!("failed to create recovery keyring entry: {e}"))
}

pub fn recovery_entry_available() -> bool {
    let Ok(entry) = recovery_entry() else {
        return false;
    };
    match entry.get_password() {
        Ok(_) => true,
        Err(KeyringError::NoEntry) => true,
        Err(_) => false,
    }
}

fn store_recovery_escrow(vault_key: &[u8; 32]) -> Result<()> {
    let entry = recovery_entry()?;
    let mut escrow_key = [0u8; 32];
    rand::rng().fill_bytes(&mut escrow_key);
    let cipher = Aes256Gcm::new_from_slice(&escrow_key).context("invalid escrow key")?;
    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, vault_key.as_ref())
        .map_err(|e| anyhow!("escrow encrypt failed: {e}"))?;
    let mut wrapped = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    wrapped.extend_from_slice(&nonce_bytes);
    wrapped.extend(ciphertext);

    let blob = VaultRecoveryBlob {
        escrow_key: STANDARD.encode(escrow_key),
        wrapped_vault_key: STANDARD.encode(wrapped),
    };
    entry
        .set_password(&serde_json::to_string(&blob)?)
        .context("failed to store recovery escrow")
}

fn load_recovery_escrow() -> Result<Zeroizing<[u8; 32]>> {
    let entry = recovery_entry()?;
    let raw = entry
        .get_password()
        .map_err(|e| anyhow!("recovery escrow read failed: {e}"))?;
    let blob: VaultRecoveryBlob =
        serde_json::from_str(&raw).context("failed to parse recovery escrow")?;
    let escrow_key = STANDARD
        .decode(blob.escrow_key.trim())
        .context("invalid escrow key")?;
    if escrow_key.len() != 32 {
        return Err(anyhow!("invalid escrow key length"));
    }
    let mut key_arr = [0u8; 32];
    key_arr.copy_from_slice(&escrow_key);

    let wrapped = STANDARD
        .decode(blob.wrapped_vault_key.trim())
        .context("invalid escrow wrap")?;
    if wrapped.len() <= NONCE_LEN {
        return Err(anyhow!("escrow wrap too short"));
    }
    let cipher = Aes256Gcm::new_from_slice(&key_arr).context("invalid escrow key")?;
    let (nonce_bytes, ciphertext) = wrapped.split_at(NONCE_LEN);
    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| anyhow!("escrow decrypt failed: {e}"))?;
    if plaintext.len() != 32 {
        return Err(anyhow!("unexpected vault key length from escrow"));
    }
    let mut vault_key = Zeroizing::new([0u8; 32]);
    vault_key.copy_from_slice(&plaintext);
    Ok(vault_key)
}

fn purge_preview_cache(app: &AppHandle) -> Result<()> {
    let dir = paths::preview_cache_dir(app)?;
    if dir.exists() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                let _ = fs::remove_file(path);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_unwrap_vault_key_round_trip() {
        let vault_key = [42u8; 32];
        let salt = SaltString::generate(&mut argon2_os_rng());
        let password = "test-password-123";
        let wrapped = wrap_vault_key(&vault_key, password, salt.as_salt().as_str()).expect("wrap");
        let unwrapped =
            unwrap_vault_key(&wrapped, password, salt.as_salt().as_str()).expect("unwrap");
        assert_eq!(unwrapped.as_ref(), &vault_key);
    }

    #[test]
    fn verify_master_password_accepts_correct_password() {
        let password = "correct-horse-battery";
        let salt = SaltString::generate(&mut argon2_os_rng());
        let hash = hash_master_password(password, &salt).expect("hash");
        assert!(verify_master_password(password, &salt.to_string(), &hash).expect("verify"));
        assert!(!verify_master_password("wrong", &salt.to_string(), &hash).expect("verify wrong"));
    }

    #[test]
    fn validate_master_password_rejects_short() {
        assert!(validate_master_password("short").is_err());
        assert!(validate_master_password("long-enough").is_ok());
    }

    #[test]
    fn failed_unlock_backoff_escalates() {
        let manager = VaultManager::default();
        for _ in 0..MAX_UNLOCK_ATTEMPTS_BEFORE_BACKOFF {
            manager.register_failed_unlock();
        }
        assert!(manager.unlock_is_blocked());
    }

    impl VaultManager {
        fn unlock_is_blocked(&self) -> bool {
            self.inner
                .lock()
                .unlock_blocked_until
                .is_some_and(|until| Instant::now() < until)
        }
    }
}
