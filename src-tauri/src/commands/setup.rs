/// 平台信息、yt-dlp 和 Deno 安装管理

use crate::utils;
use futures_util::StreamExt;
use std::process::Stdio;
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncBufReadExt;

use super::{YtdlpStatus, DenoStatus};

#[cfg(target_os = "windows")]
use super::CREATE_NO_WINDOW;

// ========== 平台信息 ==========

/// 获取当前运行平台
#[tauri::command]
pub fn get_platform() -> String {
    if cfg!(target_os = "windows") {
        "windows".to_string()
    } else if cfg!(target_os = "macos") {
        "macos".to_string()
    } else {
        "linux".to_string()
    }
}

// ========== yt-dlp 管理 ==========

/// 获取 yt-dlp 安装状态和版本
#[tauri::command]
pub async fn get_ytdlp_status(app: AppHandle) -> Result<YtdlpStatus, String> {
    let ytdlp_path = utils::get_ytdlp_path(&app)?;

    if !ytdlp_path.exists() {
        return Ok(YtdlpStatus {
            installed: false,
            version: String::new(),
            path: ytdlp_path.to_string_lossy().to_string(),
        });
    }

    let mut cmd = tokio::process::Command::new(&ytdlp_path);
    cmd.arg("--version")
        .env("PYTHONUTF8", "1")
        .env("PYTHONIOENCODING", "utf-8");
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output = cmd
        .output()
        .await
        .map_err(|e| format!("运行 yt-dlp 失败: {}", e))?;

    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();

    Ok(YtdlpStatus {
        installed: true,
        version,
        path: ytdlp_path.to_string_lossy().to_string(),
    })
}

/// 下载 yt-dlp 可执行文件
#[tauri::command]
pub async fn download_ytdlp(app: AppHandle) -> Result<(), String> {
    let ytdlp_path = utils::get_ytdlp_path(&app)?;
    let url = utils::get_ytdlp_download_url();

    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("下载失败: {}", e))?;

    let total_size = response.content_length().unwrap_or(0);
    let mut downloaded: u64 = 0;

    let mut file = tokio::fs::File::create(&ytdlp_path)
        .await
        .map_err(|e| format!("创建文件失败: {}", e))?;

    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("下载错误: {}", e))?;
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk)
            .await
            .map_err(|e| format!("写入错误: {}", e))?;

        downloaded += chunk.len() as u64;
        let percent = if total_size > 0 {
            (downloaded as f64 / total_size as f64) * 100.0
        } else {
            0.0
        };

        let _ = app.emit(
            "ytdlp-download-progress",
            serde_json::json!({
                "percent": percent,
                "downloaded": downloaded,
                "total": total_size,
            }),
        );
    }

    // Unix: 设置可执行权限
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&ytdlp_path, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("设置权限失败: {}", e))?;
    }

    Ok(())
}

/// 更新 yt-dlp 到最新版本
#[tauri::command]
pub async fn update_ytdlp(app: AppHandle) -> Result<String, String> {
    let ytdlp_path = utils::get_ytdlp_path(&app)?;
    if !ytdlp_path.exists() {
        return Err("yt-dlp 未安装".to_string());
    }

    let mut cmd = tokio::process::Command::new(&ytdlp_path);
    cmd.arg("-U")
        .env("PYTHONUTF8", "1")
        .env("PYTHONIOENCODING", "utf-8");
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("启动更新失败: {}", e))?;

    let stdout = child.stdout.take().ok_or("获取 stdout 失败")?;
    let stderr = child.stderr.take().ok_or("获取 stderr 失败")?;

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
        .map_err(|e| format!("进程错误: {}", e))?;

    if status.success() {
        Ok(format!("{}\n{}", stdout_out, stderr_out).trim().to_string())
    } else {
        Err(format!("更新失败: {}", stderr_out.trim()))
    }
}

// ========== Deno 管理 ==========

/// 获取 Deno 安装状态和版本
#[tauri::command]
pub async fn get_deno_status(app: AppHandle) -> Result<DenoStatus, String> {
    let deno_path = utils::get_deno_path(&app)?;

    if !deno_path.exists() {
        return Ok(DenoStatus {
            installed: false,
            version: String::new(),
            path: deno_path.to_string_lossy().to_string(),
        });
    }

    let mut cmd = tokio::process::Command::new(&deno_path);
    cmd.arg("--version");
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output = cmd.output().await;

    match output {
        Ok(out) if out.status.success() => {
            let version_str = String::from_utf8_lossy(&out.stdout).trim().to_string();
            let version = version_str
                .lines()
                .next()
                .unwrap_or("")
                .replace("deno ", "")
                .trim()
                .to_string();
            Ok(DenoStatus {
                installed: true,
                version,
                path: deno_path.to_string_lossy().to_string(),
            })
        }
        _ => Ok(DenoStatus {
            installed: true,
            version: String::new(),
            path: deno_path.to_string_lossy().to_string(),
        }),
    }
}

/// 下载 Deno 可执行文件（从 zip 解压）
#[tauri::command]
pub async fn download_deno(app: AppHandle) -> Result<(), String> {
    let deno_path = utils::get_deno_path(&app)?;
    let url = utils::get_deno_download_url();

    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("下载失败: {}", e))?;

    let total_size = response.content_length().unwrap_or(0);
    let mut downloaded: u64 = 0;

    // 下载 zip 到临时文件
    let zip_path = deno_path.with_extension("zip");
    let mut file = tokio::fs::File::create(&zip_path)
        .await
        .map_err(|e| format!("创建临时文件失败: {}", e))?;

    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("下载错误: {}", e))?;
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk)
            .await
            .map_err(|e| format!("写入错误: {}", e))?;

        downloaded += chunk.len() as u64;
        let percent = if total_size > 0 {
            (downloaded as f64 / total_size as f64) * 100.0
        } else {
            0.0
        };
        let _ = app.emit(
            "deno-download-progress",
            serde_json::json!({
                "percent": percent,
                "downloaded": downloaded,
                "total": total_size,
            }),
        );
    }

    // 确保文件写入完成
    tokio::io::AsyncWriteExt::shutdown(&mut file)
        .await
        .map_err(|e| format!("刷新文件失败: {}", e))?;
    drop(file);

    // 解压 deno 可执行文件
    let zip_path_clone = zip_path.clone();
    let deno_path_clone = deno_path.clone();
    let deno_bin_name = if cfg!(target_os = "windows") {
        "deno.exe"
    } else {
        "deno"
    };

    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&zip_path_clone)
            .map_err(|e| format!("打开 zip 失败: {}", e))?;
        let mut archive =
            zip::ZipArchive::new(file).map_err(|e| format!("读取 zip 失败: {}", e))?;

        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| format!("读取 zip 条目失败: {}", e))?;
            let name = entry.name().to_lowercase();
            if name == deno_bin_name || name.ends_with(&format!("/{}", deno_bin_name)) {
                let mut outfile = std::fs::File::create(&deno_path_clone)
                    .map_err(|e| format!("创建 deno 可执行文件失败: {}", e))?;
                std::io::copy(&mut entry, &mut outfile)
                    .map_err(|e| format!("解压 deno 失败: {}", e))?;
                return Ok(());
            }
        }
        Err(format!("zip 中未找到 {}", deno_bin_name))
    })
    .await
    .map_err(|e| format!("任务错误: {}", e))??;

    // Unix: 设置可执行权限
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&deno_path, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("设置权限失败: {}", e))?;
    }

    // 清理 zip 文件
    let _ = tokio::fs::remove_file(&zip_path).await;

    Ok(())
}
