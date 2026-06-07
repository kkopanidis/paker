mod commands;
mod index;
mod s3;
mod storage;
mod transfer;

use index::BucketIndexManager;
use storage::ObjectCacheManager;
use tauri::Manager;
use transfer::TransferManager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(TransferManager::default())
        .manage(BucketIndexManager::default())
        .setup(|app| {
            if !storage::paths::is_portable_mode() {
                let _ = keyring::use_native_store(false);
            }

            let cache = ObjectCacheManager::new(app.handle()).unwrap_or_else(|error| {
                panic!("failed to initialize object cache: {error:#}");
            });
            app.manage(cache);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::connections::list_connections,
            commands::connections::get_connection,
            commands::connections::save_connection,
            commands::connections::delete_connection,
            commands::connections::test_connection,
            commands::s3_ops::list_buckets,
            commands::s3_ops::verify_bucket,
            commands::s3_ops::read_list_cache,
            commands::s3_ops::list_objects,
            commands::s3_ops::calculate_prefix_size,
            commands::s3_ops::get_bucket_metadata,
            commands::s3_ops::pick_upload_files,
            commands::s3_ops::upload_files,
            commands::s3_ops::download_files,
            commands::s3_ops::delete_objects,
            commands::s3_ops::rename_object,
            commands::s3_ops::create_folder,
            commands::s3_ops::head_object,
            commands::s3_ops::check_objects_exist,
            commands::s3_ops::copy_objects,
            commands::s3_ops::move_objects,
            commands::s3_ops::cancel_transfer,
            commands::s3_ops::pause_transfer,
            commands::s3_ops::resume_transfer,
            commands::local_fs::list_local_dir,
            commands::local_fs::get_home_dir,
            commands::local_fs::pick_local_folder,
            commands::local_fs::get_parent_path,
            commands::ui_state::get_last_local_dir,
            commands::ui_state::set_last_local_dir,
            commands::ui_state::get_transfer_settings,
            commands::ui_state::get_full_ui_state,
            commands::ui_state::get_connection_nav,
            commands::ui_state::set_connection_nav,
            commands::ui_state::get_bookmarks,
            commands::ui_state::add_bookmark,
            commands::ui_state::remove_bookmark,
            commands::ui_state::get_ui_preferences,
            commands::ui_state::set_ui_preferences,
            commands::ui_state::get_panel_layout,
            commands::ui_state::set_panel_layout,
            commands::s3_ops::presign_object,
            commands::s3_ops::preview_object_to_cache,
            commands::bucket_index::get_bucket_index_status,
            commands::bucket_index::start_bucket_index,
            commands::bucket_index::pause_bucket_index,
            commands::bucket_index::resume_bucket_index,
            commands::bucket_index::cancel_bucket_index,
            commands::bucket_index::search_bucket_index,
            commands::bucket_index::export_bucket_index_csv,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
