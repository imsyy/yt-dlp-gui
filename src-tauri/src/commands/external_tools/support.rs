//! 外部工具管理共用的状态探测、进度通知与原子替换逻辑。

use crate::{commands::CREATE_NO_WINDOW, utils};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

use super::{ToolProgress, ToolStatus};

/// HTTP 下载超时时间（30 分钟，用于大文件下载）
pub(super) const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(1800);

/// 为可执行文件生成同目录临时路径，确保最终替换不会跨文件系统。
pub(super) fn executable_temp_path(target: &Path, suffix: &str) -> Result<PathBuf, String> {
    let stem = target
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or("err_invalid_executable_path")?;
    let extension = target.extension().and_then(|s| s.to_str());
    let name = match extension {
        Some(ext) => format!("{}.{}.{}", stem, suffix, ext),
        None => format!("{}.{}", stem, suffix),
    };
    Ok(target.with_file_name(name))
}

/// 用已验证的临时文件替换正式文件；Windows 上保留可恢复备份，避免先删后换。
pub(super) fn replace_executable(temp_path: &Path, target_path: &Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let backup_path = executable_temp_path(target_path, "backup")?;
        let _ = std::fs::remove_file(&backup_path);
        if target_path.exists() {
            std::fs::rename(target_path, &backup_path)
                .map_err(|e| format!("err_backup_executable:{}", e))?;
        }
        if let Err(e) = std::fs::rename(temp_path, target_path) {
            if backup_path.exists() {
                let _ = std::fs::rename(&backup_path, target_path);
            }
            return Err(format!("err_replace_executable:{}", e));
        }
        let _ = std::fs::remove_file(backup_path);
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    {
        std::fs::rename(temp_path, target_path).map_err(|e| format!("err_replace_executable:{}", e))
    }
}

pub(super) fn emit_tool_progress(
    app: &AppHandle,
    tool: &str,
    operation: &str,
    stage: &str,
    percent: Option<f64>,
) {
    let _ = app.emit(
        "tool-operation-progress",
        ToolProgress {
            tool: tool.to_string(),
            operation: operation.to_string(),
            stage: stage.to_string(),
            percent,
        },
    );
}

pub(super) async fn build_tool_status(
    tool: &str,
    path: PathBuf,
    managed_path: PathBuf,
    version_arg: &str,
) -> Result<ToolStatus, String> {
    let configured_source = utils::get_tool_source(tool)?;
    let has_cli_override = utils::get_cli_tool_path(tool).is_some();
    let source = if has_cli_override {
        "custom"
    } else {
        configured_source.as_str()
    };
    let installed = path.exists();
    if !installed {
        return Ok(ToolStatus {
            installed: false,
            version: String::new(),
            path: path.to_string_lossy().to_string(),
            source: source.to_string(),
            is_managed: !has_cli_override && configured_source == utils::ToolSource::Managed,
            can_update: false,
        });
    }

    let mut cmd = tokio::process::Command::new(&path);
    cmd.arg(version_arg)
        .env("PYTHONUTF8", "1")
        .env("PYTHONIOENCODING", "utf-8");
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);
    let output = cmd
        .output()
        .await
        .map_err(|e| format!("err_run_tool:{}:{}", tool, e))?;
    let raw = if output.stdout.is_empty() {
        &output.stderr
    } else {
        &output.stdout
    };
    let first_line = String::from_utf8_lossy(raw)
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    let version = if tool == "ffmpeg" {
        first_line
            .strip_prefix("ffmpeg version ")
            .unwrap_or(&first_line)
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_string()
    } else if tool == "deno" {
        first_line
            .strip_prefix("deno ")
            .unwrap_or(&first_line)
            .to_string()
    } else {
        first_line
    };

    Ok(ToolStatus {
        installed: output.status.success(),
        version,
        path: path.to_string_lossy().to_string(),
        source: source.to_string(),
        is_managed: path == managed_path,
        can_update: !has_cli_override
            && (configured_source == utils::ToolSource::Managed || tool != "ffmpeg"),
    })
}
