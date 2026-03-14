use tauri::Emitter;
use tauri::Manager;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconEvent;

mod commands;
mod parser;
mod process;
mod utils;

#[tauri::command]
fn update_tray_menu(app: tauri::AppHandle, show_label: String, quit_label: String) -> Result<(), String> {
    if let Some(tray) = app.tray_by_id("main") {
        let show = MenuItem::with_id(&app, "show", &show_label, true, None::<&str>)
            .map_err(|e| e.to_string())?;
        let quit = MenuItem::with_id(&app, "quit", &quit_label, true, None::<&str>)
            .map_err(|e| e.to_string())?;
        let menu = Menu::with_items(&app, &[&show, &quit])
            .map_err(|e| e.to_string())?;
        tray.set_menu(Some(menu)).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
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
        .manage(commands::DownloadState::default())
        .invoke_handler(tauri::generate_handler![
            update_tray_menu,
            commands::get_platform,
            commands::set_binary_path_resolve_mode,
            commands::get_ytdlp_status,
            commands::download_ytdlp,
            commands::update_ytdlp,
            commands::get_deno_status,
            commands::download_deno,
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
