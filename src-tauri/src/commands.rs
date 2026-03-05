use crate::utils;
use futures_util::StreamExt;
use std::process::Stdio;
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncBufReadExt;

/// Windows: CREATE_NO_WINDOW flag
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

// ========== 状态结构 ==========

#[derive(serde::Serialize)]
pub struct YtdlpStatus {
    pub installed: bool,
    pub version: String,
    pub path: String,
}

#[derive(serde::Serialize)]
pub struct DenoStatus {
    pub installed: bool,
    pub version: String,
    pub path: String,
}

// ========== 平台信息 ==========

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
    cmd.arg("--version").env("PYTHONUTF8", "1");
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output = cmd
        .output()
        .await
        .map_err(|e| format!("Failed to run yt-dlp: {}", e))?;

    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();

    Ok(YtdlpStatus {
        installed: true,
        version,
        path: ytdlp_path.to_string_lossy().to_string(),
    })
}

#[tauri::command]
pub async fn download_ytdlp(app: AppHandle) -> Result<(), String> {
    let ytdlp_path = utils::get_ytdlp_path(&app)?;
    let url = utils::get_ytdlp_download_url();

    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Failed to download: {}", e))?;

    let total_size = response.content_length().unwrap_or(0);
    let mut downloaded: u64 = 0;

    let mut file = tokio::fs::File::create(&ytdlp_path)
        .await
        .map_err(|e| format!("Failed to create file: {}", e))?;

    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Download error: {}", e))?;
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk)
            .await
            .map_err(|e| format!("Write error: {}", e))?;

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
            .map_err(|e| format!("Failed to set permissions: {}", e))?;
    }

    Ok(())
}

#[tauri::command]
pub async fn update_ytdlp(app: AppHandle) -> Result<String, String> {
    let ytdlp_path = utils::get_ytdlp_path(&app)?;
    if !ytdlp_path.exists() {
        return Err("yt-dlp is not installed".to_string());
    }

    let mut cmd = tokio::process::Command::new(&ytdlp_path);
    cmd.arg("-U").env("PYTHONUTF8", "1");
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    // 使用 stdout/stderr 管道实时发送进度
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to start update: {}", e))?;

    let stdout = child.stdout.take().ok_or("Failed to capture stdout")?;
    let stderr = child.stderr.take().ok_or("Failed to capture stderr")?;

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
        .map_err(|e| format!("Process error: {}", e))?;

    if status.success() {
        Ok(format!("{}\n{}", stdout_out, stderr_out).trim().to_string())
    } else {
        Err(format!("Update failed: {}", stderr_out.trim()))
    }
}

// ========== Deno 管理 ==========

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

#[tauri::command]
pub async fn download_deno(app: AppHandle) -> Result<(), String> {
    let deno_path = utils::get_deno_path(&app)?;
    let url = utils::get_deno_download_url();

    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Failed to download: {}", e))?;

    let total_size = response.content_length().unwrap_or(0);
    let mut downloaded: u64 = 0;

    // 下载 zip 到临时文件
    let zip_path = deno_path.with_extension("zip");
    let mut file = tokio::fs::File::create(&zip_path)
        .await
        .map_err(|e| format!("Failed to create temp file: {}", e))?;

    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Download error: {}", e))?;
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk)
            .await
            .map_err(|e| format!("Write error: {}", e))?;

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
        .map_err(|e| format!("Failed to flush file: {}", e))?;
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
            .map_err(|e| format!("Failed to open zip: {}", e))?;
        let mut archive =
            zip::ZipArchive::new(file).map_err(|e| format!("Failed to read zip: {}", e))?;

        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| format!("Failed to read zip entry: {}", e))?;
            let name = entry.name().to_lowercase();
            if name == deno_bin_name || name.ends_with(&format!("/{}", deno_bin_name)) {
                let mut outfile = std::fs::File::create(&deno_path_clone)
                    .map_err(|e| format!("Failed to create deno binary: {}", e))?;
                std::io::copy(&mut entry, &mut outfile)
                    .map_err(|e| format!("Failed to extract deno: {}", e))?;
                return Ok(());
            }
        }
        Err(format!("{} not found in zip archive", deno_bin_name))
    })
    .await
    .map_err(|e| format!("Task error: {}", e))??;

    // Unix: 设置可执行权限
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&deno_path, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("Failed to set permissions: {}", e))?;
    }

    // 清理 zip 文件
    let _ = tokio::fs::remove_file(&zip_path).await;

    Ok(())
}
