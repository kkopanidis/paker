mod assistant;
mod commands;
mod error;
mod index;
mod os_auth;
mod path_safety;
mod s3;
mod storage;
mod transfer;
#[cfg(test)]
mod type_export;

pub use error::PakerError;

/// Public surface for `src-tauri/tests/` integration tests (`--features integration-tests`).
#[cfg(any(test, feature = "integration-tests"))]
pub mod test_exports {
    pub use crate::s3::client::build_client;
    pub use crate::s3::operations::*;
    pub use crate::storage::ConnectionProfile;
}

/// Public surface for the `paker-mcp` binary (`--features mcp`).
/// Exposes only read-only, credential-free types and functions.
#[cfg(feature = "mcp")]
pub mod mcp_exports {
    pub use crate::assistant::explain::{explain_error_code, ErrorExplanation};
    pub use crate::assistant::query::IndexQuery;
    pub use crate::assistant::reports::{BucketReport, PrefixStat};
    pub use crate::assistant::templates::{
        generate_cli_commands, CliCommandSuggestion, CliGenerateInput,
    };
    pub use crate::storage::bucket_index::{BucketIndexMeta, IndexedObject};
    pub use crate::storage::object_cache::ObjectCacheManager;
    pub use crate::storage::paths::{connections_path_in, index_db_path_in, is_portable_mode};
    pub use crate::storage::profiles::{
        get_connection_from, list_connections_from, ConnectionProfile,
    };
}

use assistant::audit_log::AuditLog;
use assistant::hmac_token::HmacKey;
use assistant::proposal_store::ProposalStore;
use commands::local_fs::LocalFsScope;
use index::BucketIndexManager;
use storage::{ObjectCacheManager, VaultManager};
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
        .manage(VaultManager::default())
        .manage(HmacKey::generate())
        .manage(ProposalStore::default())
        .setup(|app| -> Result<(), Box<dyn std::error::Error>> {
            let scope = LocalFsScope::new().map_err(|e| {
                tracing::error!(error = %e, "failed to initialize local FS scope");
                e
            })?;
            app.manage(scope);

            let vault = app.state::<VaultManager>();
            vault.load_from_disk(app.handle()).map_err(|e| {
                tracing::error!(error = %e, "failed to load vault metadata");
                PakerError::Internal
            })?;

            if !storage::paths::is_portable_mode() && !vault.is_enabled() {
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

            let audit_log = AuditLog::new(app.handle()).map_err(|e| {
                tracing::error!(error = %e, "failed to initialize audit log");
                Box::new(e) as Box<dyn std::error::Error>
            })?;
            app.manage(audit_log);

            #[cfg(feature = "llm")]
            {
                use assistant::llm::try_load_model;
                use crate::storage::paths::data_dir;
                if let Ok(dir) = data_dir(app.handle()) {
                    if let Some(handle) = try_load_model(&dir) {
                        app.manage(handle);
                    }
                }
            }

            Ok(())
        })
        .invoke_handler(commands::app_commands!())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
