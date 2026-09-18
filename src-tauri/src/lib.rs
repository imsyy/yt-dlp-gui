use tauri::{Emitter, Manager};

mod app;
mod commands;
mod db;
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
        .plugin(
            // 记住窗口尺寸、位置与最大化状态，下次启动自动恢复
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(
                    tauri_plugin_window_state::StateFlags::SIZE
                        | tauri_plugin_window_state::StateFlags::POSITION
                        | tauri_plugin_window_state::StateFlags::MAXIMIZED,
                )
                .build(),
        )
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_deep_link::init())
        .setup(|app| {
            // 窗口状态恢复（尤其是最大化）可能会让窗口在此时提前显示出来，
            // 这里重新隐藏，统一交给前端 bootstrap 完成后调用 show()，避免启动白屏闪烁。
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.hide();
            }
            #[cfg(any(windows, target_os = "linux"))]
            {
                use tauri_plugin_deep_link::DeepLinkExt;
                let _ = app.deep_link().register_all();
            }
            let db_state = db::init_database(app.handle())?;
            app.manage(db_state);
            tauri::async_runtime::spawn(commands::cleanup_tool_cache(app.handle().clone()));
            app::browser_bridge::start(app.handle().clone());
            app::setup_tray(app)
        })
        .manage(app::commands::CliRequestState::new(initial_request))
        .manage(app::browser_bridge::BrowserBridgeState::default())
        .manage(commands::DownloadState::default())
        .manage(platform::process::ProcessRegistry::default())
        .invoke_handler(tauri::generate_handler![
            app::commands::update_tray_menu,
            app::commands::set_tray_visible,
            app::commands::reveal_browser_extension,
            app::commands::take_cli_open_request,
            app::browser_bridge::take_browser_extension_imports,
            commands::get_platform,
            commands::set_tool_sources,
            commands::set_youtube_extractor_args,
            commands::get_ytdlp_status,
            commands::get_ytdlp_channel,
            commands::set_ytdlp_channel,
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
            commands::cancel_download,
            commands::check_files_exist,
            commands::delete_file,
            commands::clean_task_residual_files,
            commands::tool_download_thumbnail,
            commands::tool_save_thumbnail,
            commands::tool_download_subtitles,
            commands::tool_save_subtitle,
            commands::tool_download_text,
            commands::tool_save_text_to_file,
            commands::tool_start_task,
            commands::tool_get_task_state,
            commands::tool_get_running_tasks,
            commands::tool_cancel_task,
            commands::tool_get_result,
            commands::tool_read_live_chat_page,
            commands::tool_export_live_chat,
            commands::test_proxy,
            db::db_health_check,
            db::tasks::db_get_tasks,
            db::tasks::db_upsert_task,
            db::tasks::db_upsert_tasks_batch,
            db::tasks::db_delete_task,
            db::tasks::db_delete_tasks,
            db::tasks::db_clear_completed_tasks,
            db::history::db_get_history,
            db::history::db_add_history,
            db::history::db_add_history_batch,
            db::history::db_remove_history,
            db::history::db_clear_history,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // 退出前回收仍在运行的工具子进程。Windows 上父进程退出不会连带杀掉子进程，
            // 放着不管会让 yt-dlp 变成孤儿进程继续跑（抓直播弹幕时尤其明显）。
            if let tauri::RunEvent::Exit = event {
                for pid in app.state::<platform::process::ProcessRegistry>().drain() {
                    let _ = platform::process::kill_process(pid);
                }
            }
        });
}
