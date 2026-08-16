use tauri::{Emitter, Manager};

mod app;
mod commands;
mod platform;
mod utils;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let initial_cli = app::cli::parse_cli_args(
        std::env::args_os().map(|argument| argument.to_string_lossy().to_string()),
    );
    if let Some(path) = initial_cli.ytdlp_path.clone() {
        let _ = utils::set_cli_tool_path("yt-dlp", path);
    }
    if let Some(path) = initial_cli.deno_path.clone() {
        let _ = utils::set_cli_tool_path("deno", path);
    }
    let initial_request = (!initial_cli.request.is_empty()).then_some(initial_cli.request);

    tauri::Builder::default()
        // 必须最先注册，确保协议唤醒产生的第二实例参数不会被其他插件抢先处理。
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            let cli_options = app::cli::parse_cli_args(args.iter().cloned());
            if let Some(path) = cli_options.ytdlp_path {
                let _ = utils::set_cli_tool_path("yt-dlp", path);
            }
            if let Some(path) = cli_options.deno_path {
                let _ = utils::set_cli_tool_path("deno", path);
            }
            if !cli_options.request.is_empty() {
                let _ = app.emit("cli-open-request", cli_options.request);
            }
            // 将深链接 URL 转发到前端
            for arg in &args {
                if arg.starts_with("ytdlp-gui://") {
                    let _ = app.emit("deep-link-url", arg.clone());
                }
            }
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_deep_link::init())
        .setup(|app| {
            app::browser_bridge::start(app.handle().clone());
            app::setup_tray(app)
        })
        .manage(app::commands::CliRequestState::new(initial_request))
        .manage(app::browser_bridge::BrowserBridgeState::default())
        .manage(commands::DownloadState::default())
        .invoke_handler(tauri::generate_handler![
            app::commands::update_tray_menu,
            app::commands::reveal_browser_extension,
            app::commands::take_cli_open_request,
            app::browser_bridge::take_browser_extension_imports,
            commands::get_platform,
            commands::set_tool_sources,
            commands::set_youtube_extractor_args,
            commands::get_ytdlp_status,
            commands::check_tool_update,
            commands::download_ytdlp,
            commands::update_ytdlp,
            commands::get_deno_status,
            commands::download_deno,
            commands::update_deno,
            commands::get_ffmpeg_status,
            commands::download_ffmpeg,
            commands::update_ffmpeg,
            commands::check_plugin_installed,
            commands::install_plugin,
            commands::uninstall_plugin,
            commands::save_cookie_text,
            commands::fetch_video_info,
            commands::start_download,
            commands::pause_download,
            commands::resume_download,
            commands::cancel_download,
            commands::check_files_exist,
            commands::delete_file,
            commands::tool_download_thumbnail,
            commands::tool_fetch_thumbnails,
            commands::tool_save_thumbnail,
            commands::tool_download_subtitles,
            commands::tool_fetch_subtitles,
            commands::tool_save_subtitle,
            commands::tool_download_text,
            commands::tool_save_text_to_file,
            commands::tool_fetch_live_chat,
            commands::tool_fetch_chapters,
            commands::tool_fetch_comments,
            commands::test_proxy,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
