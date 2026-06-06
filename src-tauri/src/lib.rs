mod commands;
mod s3;
mod storage;
mod transfer;

use transfer::TransferManager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(TransferManager::default())
        .invoke_handler(tauri::generate_handler![
            commands::connections::list_connections,
            commands::connections::get_connection,
            commands::connections::save_connection,
            commands::connections::delete_connection,
            commands::connections::test_connection,
            commands::s3_ops::list_buckets,
            commands::s3_ops::verify_bucket,
            commands::s3_ops::list_objects,
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
