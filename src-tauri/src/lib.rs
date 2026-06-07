mod commands;
mod error;
mod index;
mod path_safety;
mod s3;
mod storage;
mod transfer;
#[cfg(test)]
mod type_export;

pub use error::PakerError;

use commands::local_fs::LocalFsScope;
use index::BucketIndexManager;
use storage::ObjectCacheManager;
use tauri::Manager;
use transfer::TransferManager;

fn init_tracing() {
    use tracing_subscriber::EnvFilter;

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("paker=info,tauri=warn"));

    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .try_init();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_tracing();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(TransferManager::default())
        .manage(BucketIndexManager::default())
        .setup(|app| -> Result<(), Box<dyn std::error::Error>> {
            let scope = LocalFsScope::new().map_err(|e| {
                tracing::error!(error = %e, "failed to initialize local FS scope");
                e
            })?;
            app.manage(scope);

            if !storage::paths::is_portable_mode() {
                let _ = keyring::use_native_store(false);
                storage::secrets::migrate_legacy_secrets(app.handle()).map_err(|e| {
                    tracing::error!(error = %e, "failed to migrate legacy secrets");
                    PakerError::Internal
                })?;
            }

            let cache = ObjectCacheManager::new(app.handle()).map_err(|e| {
                tracing::error!(error = %e, "failed to initialize object cache");
                PakerError::Internal
            })?;
            app.manage(cache);
            Ok(())
        })
        .invoke_handler(commands::app_commands!())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
