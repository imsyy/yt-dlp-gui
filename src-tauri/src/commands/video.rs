/// 视频信息获取与 Cookie 管理

use crate::utils;
use serde_json::Value;
use tauri::AppHandle;

#[cfg(target_os = "windows")]
use super::CREATE_NO_WINDOW;

// ========== Cookie 管理 ==========

/// 保存 Cookie 文本（Netscape 格式）到应用数据目录
#[tauri::command]
pub async fn save_cookie_text(app: AppHandle, text: String) -> Result<String, String> {
    let cookie_path = utils::get_cookie_path(&app)?;
    tokio::fs::write(&cookie_path, text.as_bytes())
        .await
        .map_err(|e| format!("保存 Cookie 文件失败: {}", e))?;
    Ok(cookie_path.to_string_lossy().to_string())
}

// ========== 视频信息 ==========

/// 使用 yt-dlp -J 获取视频元信息（标题、格式列表、字幕等）
#[tauri::command]
pub async fn fetch_video_info(
    app: AppHandle,
    url: String,
    cookie_file: Option<String>,
) -> Result<Value, String> {
    let ytdlp_path = utils::get_ytdlp_path(&app)?;
    if !ytdlp_path.exists() {
        return Err("yt-dlp 未安装，请先在设置中下载".to_string());
    }

    let mut args = vec![
        "-J".to_string(),               // -J 已隐含 simulate，无需 --no-download
        "--ignore-config".to_string(),   // 忽略用户系统配置
        "--color".to_string(), "never".to_string(),
    ];
    args.extend(utils::build_js_runtime_args(&app));

    if let Some(ref cf) = cookie_file {
        if !cf.is_empty() {
            args.push("--cookies".to_string());
            args.push(cf.clone());
        }
    }

    args.push(url);

    let mut cmd = tokio::process::Command::new(&ytdlp_path);
    cmd.args(&args)
        .env("PYTHONUTF8", "1")
        .env("PYTHONIOENCODING", "utf-8");
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output = cmd
        .output()
        .await
        .map_err(|e| format!("运行 yt-dlp 失败: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    // 优先从 stdout 解析 JSON（yt-dlp 可能在 stderr 输出警告但仍成功）
    if let Some(json_str) = stdout.lines().find(|line| line.trim_start().starts_with('{')) {
        return serde_json::from_str(json_str)
            .map_err(|e| format!("解析视频信息失败: {}", e));
    }

    // 未找到 JSON，从 stderr 提取 ERROR 行作为错误信息
    let stderr = String::from_utf8_lossy(&output.stderr);
    let error_lines: Vec<&str> = stderr
        .lines()
        .filter(|l| l.contains("ERROR:"))
        .collect();
    let msg = if error_lines.is_empty() {
        stderr.trim().to_string()
    } else {
        error_lines.join("\n")
    };
    Err(msg)
}
