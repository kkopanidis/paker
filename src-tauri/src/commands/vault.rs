use crate::error::PakerError;
use crate::storage::{VaultManager, VaultStatus};
use serde::Deserialize;
use tauri::{AppHandle, Manager};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupVaultInput {
    pub master_password: String,
    #[serde(default = "default_auto_lock")]
    pub auto_lock_minutes: u32,
    #[serde(default)]
    pub lock_on_blur: bool,
}

fn default_auto_lock() -> u32 {
    15
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeMasterKeyInput {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetMasterKeyInput {
    pub new_password: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetVaultPreferencesInput {
    pub auto_lock_minutes: u32,
    pub lock_on_blur: bool,
}

#[tauri::command]
pub async fn get_vault_status(app: AppHandle) -> Result<VaultStatus, PakerError> {
    let vault = app.state::<VaultManager>();
    vault.status(&app).map_err(|_| PakerError::Internal)
}

#[tauri::command]
pub async fn setup_vault(app: AppHandle, input: SetupVaultInput) -> Result<(), PakerError> {
    let vault = app.state::<VaultManager>();
    if vault.is_enabled() {
        return Err(PakerError::InvalidInput(
            "Vault is already enabled".to_string(),
        ));
    }
    vault
        .setup(
            &app,
            &input.master_password,
            input.auto_lock_minutes,
            input.lock_on_blur,
        )
        .map_err(|e| PakerError::InvalidInput(e.to_string()))
}

#[tauri::command]
pub async fn unlock_vault(app: AppHandle, master_password: String) -> Result<(), PakerError> {
    let vault = app.state::<VaultManager>();
    vault.unlock(&app, &master_password)
}

#[tauri::command]
pub async fn lock_vault(app: AppHandle) -> Result<(), PakerError> {
    let vault = app.state::<VaultManager>();
    vault.lock(&app).map_err(|_| PakerError::Internal)
}

#[tauri::command]
pub async fn change_master_key(
    app: AppHandle,
    input: ChangeMasterKeyInput,
) -> Result<(), PakerError> {
    let vault = app.state::<VaultManager>();
    vault.change_master_key(&app, &input.current_password, &input.new_password)
}

#[tauri::command]
pub async fn reset_master_key_with_os_auth(
    app: AppHandle,
    input: ResetMasterKeyInput,
) -> Result<(), PakerError> {
    let vault = app.state::<VaultManager>();
    vault.reset_master_key_with_os_auth(&app, &input.new_password)
}

#[tauri::command]
pub async fn set_vault_preferences(
    app: AppHandle,
    input: SetVaultPreferencesInput,
) -> Result<(), PakerError> {
    let vault = app.state::<VaultManager>();
    vault.set_preferences(&app, input.auto_lock_minutes, input.lock_on_blur)
}

#[tauri::command]
pub async fn record_vault_activity(app: AppHandle) -> Result<(), PakerError> {
    let vault = app.state::<VaultManager>();
    vault.record_activity();
    if let Ok(status) = vault.status(&app) {
        if status.enabled
            && !status.locked
            && status.auto_lock_minutes > 0
            && vault.idle_lock_elapsed(status.auto_lock_minutes)
        {
            vault.lock(&app).map_err(|_| PakerError::Internal)?;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn dismiss_vault_prompt(app: AppHandle) -> Result<(), PakerError> {
    crate::storage::ui_state::set_vault_prompt_dismissed(&app, true)
        .map_err(|_| PakerError::Internal)
}

#[tauri::command]
pub async fn get_vault_prompt_dismissed(app: AppHandle) -> Result<bool, PakerError> {
    crate::storage::ui_state::get_vault_prompt_dismissed(&app).map_err(|_| PakerError::Internal)
}
