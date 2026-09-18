//! yt-dlp 的探测、安装和升级。

use crate::utils;
#[cfg(target_os = "windows")]
use crate::commands::CREATE_NO_WINDOW;
use std::process::Stdio;
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncBufReadExt;

use super::support::{
    build_tool_status, download_to_file_with_progress, emit_tool_progress, executable_temp_path,
    replace_executable, verify_executable,
};
use super::ToolStatus;

/// 获取 yt-dlp 安装状态和版本
#[tauri::command]
pub async fn get_ytdlp_status(app: AppHandle) -> Result<ToolStatus, String> {
    let ytdlp_path = utils::get_ytdlp_path(&app)?;
    let managed_path = utils::get_managed_ytdlp_path(&app)?;
    build_tool_status("yt-dlp", ytdlp_path, managed_path, "--version").await
}

/// 获取当前选择的 yt-dlp 发行通道（stable/nightly/master）
#[tauri::command]
pub fn get_ytdlp_channel() -> String {
    utils::get_ytdlp_channel().as_str().to_string()
}

/// 设置内置 yt-dlp 的发行通道；后续下载与更新检查都使用该通道的构建仓库。
#[tauri::command]
pub fn set_ytdlp_channel(channel: String) -> Result<(), String> {
    utils::set_ytdlp_channel(&channel)
}

async fn download_ytdlp_impl(app: AppHandle, operation: &str) -> Result<(), String> {
    emit_tool_progress(&app, "yt-dlp", operation, "downloading", Some(0.0));
    let ytdlp_path = utils::get_managed_ytdlp_path(&app)?;
    let temp_path = executable_temp_path(&ytdlp_path, "download")?;

    download_to_file_with_progress(
        &app,
        "yt-dlp",
        operation,
        &utils::get_ytdlp_download_url(),
        &temp_path,
        0.0,
        100.0,
    )
    .await?;

    // Unix: 设置可执行权限
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&temp_path, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("err_set_permissions:{}", e))?;
    }

    // PyInstaller 可执行文件只有真正启动后才能确认内嵌归档完整。
    if let Err(detail) = verify_executable(&temp_path, "--version").await {
        let _ = tokio::fs::remove_file(&temp_path).await;
        return Err(format!("err_validate_ytdlp:{}", detail));
    }

    emit_tool_progress(&app, "yt-dlp", operation, "installing", None);
    replace_executable(&temp_path, &ytdlp_path)?;
    emit_tool_progress(&app, "yt-dlp", operation, "complete", Some(100.0));

    Ok(())
}

/// 下载 yt-dlp 可执行文件
#[tauri::command]
pub async fn download_ytdlp(app: AppHandle) -> Result<(), String> {
    download_ytdlp_impl(app, "install").await
}

/// 更新当前选择的 yt-dlp；应用版本原子替换，系统版本使用其内置更新器。
#[tauri::command]
pub async fn update_ytdlp(app: AppHandle) -> Result<String, String> {
    let source = utils::get_tool_source("yt-dlp")?;
    if source == utils::ToolSource::Managed {
        download_ytdlp_impl(app, "update").await?;
        return Ok("Updated managed yt-dlp".to_string());
    }

    let ytdlp_path = utils::get_ytdlp_path(&app)?;
    if !ytdlp_path.exists() {
        return Err("err_ytdlp_not_installed".to_string());
    }

    emit_tool_progress(&app, "yt-dlp", "update", "updating", None);

    let mut cmd = tokio::process::Command::new(&ytdlp_path);
    // 系统版本使用其内置更新器；非 stable 通道用 --update-to 切换并更新，
    // stable 通道保持原来的 -U 行为。
    let channel = utils::get_ytdlp_channel();
    if channel == utils::YtdlpChannel::Stable {
        cmd.arg("-U");
    } else {
        cmd.arg("--update-to").arg(channel.as_str());
    }
    cmd.env("PYTHONUTF8", "1")
        .env("PYTHONIOENCODING", "utf-8");
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| format!("err_start_update:{}", e))?;

    let stdout = child.stdout.take().ok_or("err_capture_stdout")?;
    let stderr = child.stderr.take().ok_or("err_capture_stderr")?;

    let app_clone = app.clone();
    let stdout_handle = tokio::spawn(async move {
        let reader = tokio::io::BufReader::new(stdout);
        let mut lines = reader.lines();
        let mut output = String::new();
        while let Ok(Some(line)) = lines.next_line().await {
            let _ = app_clone.emit("ytdlp-update-log", &line);
            output.push_str(&line);
            output.push('\n');
        }
        output
    });

    let app_clone2 = app.clone();
    let stderr_handle = tokio::spawn(async move {
        let reader = tokio::io::BufReader::new(stderr);
        let mut lines = reader.lines();
        let mut output = String::new();
        while let Ok(Some(line)) = lines.next_line().await {
            let _ = app_clone2.emit("ytdlp-update-log", &line);
            output.push_str(&line);
            output.push('\n');
        }
        output
    });

    let stdout_out = stdout_handle.await.unwrap_or_default();
    let stderr_out = stderr_handle.await.unwrap_or_default();

    let status = child
        .wait()
        .await
        .map_err(|e| format!("err_process:{}", e))?;

    if status.success() {
        emit_tool_progress(&app, "yt-dlp", "update", "complete", Some(100.0));
        Ok(format!("{}\n{}", stdout_out, stderr_out).trim().to_string())
    } else {
        Err(format!("err_update_failed:{}", stderr_out.trim()))
    }
}
