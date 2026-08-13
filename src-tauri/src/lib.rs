use std::sync::Mutex;
use tauri::menu::{Menu, MenuItem};
use tauri::path::BaseDirectory;
use tauri::tray::TrayIconEvent;
use tauri::Emitter;
use tauri::Manager;
use tauri_plugin_opener::OpenerExt;

mod cli;
mod commands;
mod parser;
mod process;
mod utils;

#[derive(Default)]
struct CliRequestState(Mutex<Option<cli::CliOpenRequest>>);

#[tauri::command]
fn take_cli_open_request(
    state: tauri::State<'_, CliRequestState>,
) -> Result<Option<cli::CliOpenRequest>, String> {
    state
        .0
        .lock()
        .map(|mut request| request.take())
        .map_err(|e| format!("err_cli_request_state:{}", e))
}

/// Reveal the bundled browser-extension folder in the OS file manager
/// and return the absolute path.
#[tauri::command]
fn reveal_browser_extension(app: tauri::AppHandle) -> Result<String, String> {
    let path = app
        .path()
        .resolve("browser-extension", BaseDirectory::Resource)
        .map_err(|e| e.to_string())?;
    let path_str = path.to_string_lossy().into_owned();
    app.opener()
        .open_path(path_str.clone(), None::<&str>)
        .map_err(|e| e.to_string())?;
    Ok(path_str)
}

#[tauri::command]
fn update_tray_menu(
    app: tauri::AppHandle,
    show_label: String,
    quit_label: String,
) -> Result<(), String> {
    if let Some(tray) = app.tray_by_id("main") {
        let show = MenuItem::with_id(&app, "show", &show_label, true, None::<&str>)
            .map_err(|e| e.to_string())?;
        let quit = MenuItem::with_id(&app, "quit", &quit_label, true, None::<&str>)
            .map_err(|e| e.to_string())?;
        let menu = Menu::with_items(&app, &[&show, &quit]).map_err(|e| e.to_string())?;
        tray.set_menu(Some(menu)).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let initial_cli = cli::parse_cli_args(
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
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            let cli_options = cli::parse_cli_args(args.iter().cloned());
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
        .setup(|app| {
            let show = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;

            if let Some(tray) = app.tray_by_id("main") {
                tray.set_menu(Some(menu))?;
                tray.on_menu_event(move |app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.unminimize();
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "quit" => {
                        // Emit event to frontend, let it decide whether to confirm
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.unminimize();
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                        let _ = app.emit("tray-quit-requested", ());
                    }
                    _ => {}
                });
                tray.on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { button, .. } = event {
                        if button == tauri::tray::MouseButton::Left {
                            if let Some(w) = tray.app_handle().get_webview_window("main") {
                                let _ = w.unminimize();
                                let _ = w.show();
                                let _ = w.set_focus();
                            }
                        }
                    }
                });
            }

            Ok(())
        })
        .manage(CliRequestState(Mutex::new(initial_request)))
        .manage(commands::DownloadState::default())
        .invoke_handler(tauri::generate_handler![
            update_tray_menu,
            reveal_browser_extension,
            take_cli_open_request,
            commands::get_platform,
            commands::set_tool_sources,
            commands::set_youtube_extractor_args,
            commands::get_ytdlp_status,
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
