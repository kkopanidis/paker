mod commands;
mod s3;
mod storage;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
