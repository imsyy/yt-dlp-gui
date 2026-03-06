mod commands;
mod parser;
mod process;
mod utils;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(commands::DownloadState::default())
        .invoke_handler(tauri::generate_handler![
            commands::get_platform,
            commands::get_ytdlp_status,
            commands::download_ytdlp,
            commands::update_ytdlp,
            commands::get_deno_status,
            commands::download_deno,
            commands::save_cookie_text,
            commands::fetch_video_info,
            commands::start_download,
            commands::pause_download,
            commands::resume_download,
            commands::cancel_download,
            commands::check_files_exist,
            commands::delete_file,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
