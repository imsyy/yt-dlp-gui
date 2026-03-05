use crate::utils;
use futures_util::StreamExt;
use serde_json::Value;
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
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

// ========== 下载状态管理 ==========

pub struct DownloadProcessInfo {
    pub pid: u32,
    pub cancelled: bool,
    pub output_files: Vec<String>,
}

pub struct DownloadState {
    pub processes: Arc<Mutex<HashMap<String, DownloadProcessInfo>>>,
}

impl Default for DownloadState {
    fn default() -> Self {
        Self {
            processes: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadParams {
    pub id: String,
    pub url: String,
    pub download_dir: String,
    pub download_mode: String,
    pub video_format: Option<String>,
    pub audio_format: Option<String>,
    pub cookie_file: Option<String>,
    pub embed_subs: bool,
    pub embed_thumbnail: bool,
    pub embed_metadata: bool,
    pub no_merge: bool,
    pub recode_format: Option<String>,
    pub limit_rate: Option<String>,
    pub subtitles: Vec<String>,
    pub start_time: Option<f64>,
    pub end_time: Option<f64>,
    pub no_playlist: bool,
    pub playlist_items: Option<String>,
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
    cmd.arg("--version")
        .env("PYTHONUTF8", "1")
        .env("PYTHONIOENCODING", "utf-8");
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
    cmd.arg("-U")
        .env("PYTHONUTF8", "1")
        .env("PYTHONIOENCODING", "utf-8");
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

// ========== Cookie ==========

#[tauri::command]
pub async fn save_cookie_text(app: AppHandle, text: String) -> Result<String, String> {
    let cookie_path = utils::get_cookie_path(&app)?;
    tokio::fs::write(&cookie_path, text.as_bytes())
        .await
        .map_err(|e| format!("Failed to save cookie file: {}", e))?;
    Ok(cookie_path.to_string_lossy().to_string())
}

// ========== 视频信息 ==========

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

    let mut args = vec!["-J".to_string(), "--no-download".to_string()];
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
        .map_err(|e| format!("Failed to run yt-dlp: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Try to parse JSON from stdout first (yt-dlp may succeed even with warnings on stderr)
    if let Some(json_str) = stdout.lines().find(|line| line.trim_start().starts_with('{')) {
        return serde_json::from_str(json_str)
            .map_err(|e| format!("解析视频信息失败: {}", e));
    }

    // No JSON found — extract ERROR lines from stderr for a cleaner message
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

// ========== 进度解析 ==========

struct ProgressInfo {
    percent: f64,
    speed: String,
    eta: String,
    downloaded: String,
    total: String,
}

fn parse_progress_line(line: &str) -> Option<ProgressInfo> {
    if !line.contains("[download]") || !line.contains('%') {
        return None;
    }
    let percent_pos = line.find('%')?;
    let before = &line[..percent_pos];
    let percent_str = before.split_whitespace().last()?;
    let percent: f64 = percent_str.parse().ok()?;

    // Parse total size: "of ~ 50.35MiB" or "of 50.35MiB"
    let total = if let Some(of_pos) = line.find("% of") {
        let after_of = &line[of_pos + 4..];
        let size_part = after_of.trim().trim_start_matches("~").trim();
        size_part
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_string()
    } else {
        String::new()
    };

    // Calculate downloaded from percent and total
    let downloaded = if !total.is_empty() && percent > 0.0 {
        if let Some(total_bytes) = parse_size_to_bytes(&total) {
            let dl_bytes = (total_bytes * percent / 100.0) as u64;
            format_bytes(dl_bytes)
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    let speed = if let Some(at_pos) = line.find(" at ") {
        let after_at = &line[at_pos + 4..];
        if let Some(eta_pos) = after_at.find(" ETA ") {
            after_at[..eta_pos].trim().to_string()
        } else {
            after_at.split_whitespace().next().unwrap_or("").to_string()
        }
    } else {
        String::new()
    };

    let eta = if let Some(eta_pos) = line.find("ETA ") {
        line[eta_pos + 4..].trim().to_string()
    } else {
        String::new()
    };

    Some(ProgressInfo {
        percent,
        speed,
        eta,
        downloaded,
        total,
    })
}

fn parse_size_to_bytes(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.ends_with("GiB") {
        s.trim_end_matches("GiB").trim().parse::<f64>().ok().map(|v| v * 1073741824.0)
    } else if s.ends_with("MiB") {
        s.trim_end_matches("MiB").trim().parse::<f64>().ok().map(|v| v * 1048576.0)
    } else if s.ends_with("KiB") {
        s.trim_end_matches("KiB").trim().parse::<f64>().ok().map(|v| v * 1024.0)
    } else if s.ends_with("B") {
        s.trim_end_matches("B").trim().parse::<f64>().ok()
    } else {
        None
    }
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KiB", "MiB", "GiB"];
    let mut size = bytes as f64;
    let mut i = 0;
    while size >= 1024.0 && i < UNITS.len() - 1 {
        size /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{:.0}{}", size, UNITS[i])
    } else {
        format!("{:.1}{}", size, UNITS[i])
    }
}

fn parse_destination(line: &str) -> Option<String> {
    let trimmed = line.trim();
    // [download] Destination: /path/to/file.ext
    if let Some(rest) = trimmed.strip_prefix("[download] Destination: ") {
        return Some(rest.trim().to_string());
    }
    // [download] /path/to/file.ext has already been downloaded
    if trimmed.starts_with("[download] ") && trimmed.ends_with("has already been downloaded") {
        let inner = trimmed
            .strip_prefix("[download] ")?
            .strip_suffix("has already been downloaded")?
            .trim();
        if !inner.is_empty() {
            return Some(inner.to_string());
        }
    }
    // [Merger] Merging formats into "file.ext"
    if trimmed.contains("[Merger] Merging formats into") {
        let start = trimmed.find('"')? + 1;
        let end = trimmed.rfind('"')?;
        if start < end {
            return Some(trimmed[start..end].to_string());
        }
    }
    None
}

/// 处理 yt-dlp 的一行输出：解析进度、跟踪文件、发送事件
fn process_output_line(
    app: &AppHandle,
    task_id: &str,
    processes: &Arc<Mutex<HashMap<String, DownloadProcessInfo>>>,
    line: &str,
) {
    if let Some(info) = parse_progress_line(line) {
        let _ = app.emit(
            "download-progress",
            serde_json::json!({
                "id": task_id,
                "percent": info.percent,
                "speed": info.speed,
                "eta": info.eta,
                "downloaded": info.downloaded,
                "total": info.total,
            }),
        );
    }
    if let Some(dest) = parse_destination(line) {
        if let Ok(mut map) = processes.lock() {
            if let Some(info) = map.get_mut(task_id) {
                info.output_files.push(dest);
            }
        }
    }
    let _ = app.emit(
        "download-log",
        serde_json::json!({ "id": task_id, "line": line }),
    );
}

// ========== 进程控制 (Windows) ==========

#[cfg(target_os = "windows")]
mod win32 {
    #[repr(C)]
    pub struct THREADENTRY32 {
        pub dw_size: u32,
        pub cnt_usage: u32,
        pub th32_thread_id: u32,
        pub th32_owner_process_id: u32,
        pub tp_base_pri: i32,
        pub tp_delta_pri: i32,
        pub dw_flags: u32,
    }

    pub const TH32CS_SNAPTHREAD: u32 = 0x00000004;
    pub const THREAD_SUSPEND_RESUME: u32 = 0x0002;

    extern "system" {
        pub fn CreateToolhelp32Snapshot(dw_flags: u32, th32_process_id: u32) -> isize;
        pub fn Thread32First(h_snapshot: isize, lpte: *mut THREADENTRY32) -> i32;
        pub fn Thread32Next(h_snapshot: isize, lpte: *mut THREADENTRY32) -> i32;
        pub fn OpenThread(
            dw_desired_access: u32,
            b_inherit_handle: i32,
            dw_thread_id: u32,
        ) -> isize;
        pub fn SuspendThread(h_thread: isize) -> u32;
        pub fn ResumeThread(h_thread: isize) -> u32;
        pub fn CloseHandle(h_object: isize) -> i32;
    }
}

#[cfg(target_os = "windows")]
fn suspend_process_by_pid(pid: u32) -> Result<(), String> {
    unsafe {
        use win32::*;
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
        if snapshot == -1 {
            return Err("Failed to create thread snapshot".into());
        }
        let mut entry = std::mem::zeroed::<THREADENTRY32>();
        entry.dw_size = std::mem::size_of::<THREADENTRY32>() as u32;
        if Thread32First(snapshot, &mut entry) != 0 {
            loop {
                if entry.th32_owner_process_id == pid {
                    let thread = OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32_thread_id);
                    if thread != 0 {
                        SuspendThread(thread);
                        CloseHandle(thread);
                    }
                }
                if Thread32Next(snapshot, &mut entry) == 0 {
                    break;
                }
            }
        }
        CloseHandle(snapshot);
        Ok(())
    }
}

#[cfg(not(target_os = "windows"))]
fn suspend_process_by_pid(pid: u32) -> Result<(), String> {
    std::process::Command::new("kill")
        .args(["-STOP", &pid.to_string()])
        .output()
        .map_err(|e| format!("Failed to suspend: {}", e))?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn resume_process_by_pid(pid: u32) -> Result<(), String> {
    unsafe {
        use win32::*;
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
        if snapshot == -1 {
            return Err("Failed to create thread snapshot".into());
        }
        let mut entry = std::mem::zeroed::<THREADENTRY32>();
        entry.dw_size = std::mem::size_of::<THREADENTRY32>() as u32;
        if Thread32First(snapshot, &mut entry) != 0 {
            loop {
                if entry.th32_owner_process_id == pid {
                    let thread = OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32_thread_id);
                    if thread != 0 {
                        ResumeThread(thread);
                        CloseHandle(thread);
                    }
                }
                if Thread32Next(snapshot, &mut entry) == 0 {
                    break;
                }
            }
        }
        CloseHandle(snapshot);
        Ok(())
    }
}

#[cfg(not(target_os = "windows"))]
fn resume_process_by_pid(pid: u32) -> Result<(), String> {
    std::process::Command::new("kill")
        .args(["-CONT", &pid.to_string()])
        .output()
        .map_err(|e| format!("Failed to resume: {}", e))?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn kill_process_by_pid(pid: u32) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    std::process::Command::new("taskkill")
        .args(["/F", "/T", "/PID", &pid.to_string()])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("Failed to kill process: {}", e))?;
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn kill_process_by_pid(pid: u32) -> Result<(), String> {
    std::process::Command::new("kill")
        .args(["-9", &pid.to_string()])
        .output()
        .map_err(|e| format!("Failed to kill process: {}", e))?;
    Ok(())
}

// ========== 视频下载 ==========

#[tauri::command]
pub async fn start_download(
    app: AppHandle,
    state: tauri::State<'_, DownloadState>,
    params: DownloadParams,
) -> Result<(), String> {
    let ytdlp_path = utils::get_ytdlp_path(&app)?;
    if !ytdlp_path.exists() {
        return Err("yt-dlp 未安装，请先在设置中下载".to_string());
    }

    let mut args: Vec<String> = vec!["--newline".to_string()];

    // JS runtime
    args.extend(utils::build_js_runtime_args(&app));

    // Format selection
    match params.download_mode.as_str() {
        "video" => {
            if let Some(ref vf) = params.video_format {
                if !vf.is_empty() {
                    args.push("-f".to_string());
                    args.push(vf.clone());
                }
            }
        }
        "audio" => {
            if let Some(ref af) = params.audio_format {
                if !af.is_empty() {
                    args.push("-f".to_string());
                    args.push(af.clone());
                }
            }
        }
        _ => {
            let vf = params
                .video_format
                .as_deref()
                .filter(|s| !s.is_empty());
            let af = params
                .audio_format
                .as_deref()
                .filter(|s| !s.is_empty());
            match (vf, af) {
                (Some(v), Some(a)) => {
                    args.push("-f".to_string());
                    args.push(format!("{}+{}", v, a));
                }
                (Some(v), None) => {
                    args.push("-f".to_string());
                    args.push(format!("{}+bestaudio", v));
                }
                (None, Some(a)) => {
                    args.push("-f".to_string());
                    args.push(format!("bestvideo+{}", a));
                }
                _ => {}
            }
        }
    }

    // Output path: use yt-dlp title template with restricted filenames for Windows safety
    let output_template = std::path::PathBuf::from(&params.download_dir)
        .join("%(title).200s.%(ext)s")
        .to_string_lossy()
        .to_string();
    args.push("-o".to_string());
    args.push(output_template);
    args.push("--windows-filenames".to_string());

    // Cookie
    if let Some(ref cf) = params.cookie_file {
        if !cf.is_empty() {
            args.push("--cookies".to_string());
            args.push(cf.clone());
        }
    }

    // Extra options
    if params.embed_subs {
        args.push("--embed-subs".to_string());
    }
    if params.embed_thumbnail {
        args.push("--embed-thumbnail".to_string());
    }
    if params.embed_metadata {
        args.push("--embed-metadata".to_string());
    }
    if params.no_merge {
        args.push("--no-merge-output".to_string());
    }
    if let Some(ref fmt) = params.recode_format {
        if !fmt.is_empty() {
            args.push("--recode-video".to_string());
            args.push(fmt.clone());
        }
    }
    if let Some(ref rate) = params.limit_rate {
        if !rate.is_empty() {
            args.push("-r".to_string());
            args.push(rate.clone());
        }
    }

    // Subtitles
    if !params.subtitles.is_empty() {
        args.push("--write-subs".to_string());
        args.push("--sub-langs".to_string());
        args.push(params.subtitles.join(","));
    }

    // Time range
    if params.start_time.is_some() || params.end_time.is_some() {
        let start = params.start_time.unwrap_or(0.0);
        let end_str = params
            .end_time
            .map(|t| format!("{}", t))
            .unwrap_or_else(|| "inf".to_string());
        args.push("--download-sections".to_string());
        args.push(format!("*{}-{}", start, end_str));
    }

    // Playlist
    if params.no_playlist {
        args.push("--no-playlist".to_string());
    } else if let Some(ref items) = params.playlist_items {
        if !items.is_empty() {
            args.push("--playlist-items".to_string());
            args.push(items.clone());
        }
    }

    // URL
    args.push(params.url);

    // Spawn process
    let mut cmd = tokio::process::Command::new(&ytdlp_path);
    cmd.args(&args)
        .env("PYTHONUTF8", "1")
        .env("PYTHONIOENCODING", "utf-8");
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("启动下载失败: {}", e))?;

    let pid = child.id().ok_or("获取进程 ID 失败")?;
    let task_id = params.id.clone();

    // Store process info
    let processes = state.processes.clone();
    {
        let mut map = processes.lock().map_err(|e| e.to_string())?;
        map.insert(
            task_id.clone(),
            DownloadProcessInfo {
                pid,
                cancelled: false,
                output_files: Vec::new(),
            },
        );
    }

    let stdout = child.stdout.take().ok_or("Failed to capture stdout")?;
    let stderr = child.stderr.take().ok_or("Failed to capture stderr")?;

    // Read stdout (raw bytes, lossy decode to handle GBK/mixed encoding on Windows)
    let app_out = app.clone();
    let id_out = task_id.clone();
    let procs_out = processes.clone();
    tokio::spawn(async move {
        let mut reader = tokio::io::BufReader::new(stdout);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match tokio::io::AsyncBufReadExt::read_until(&mut reader, b'\n', &mut buf).await {
                Ok(0) => break,
                Ok(_) => {
                    let line = String::from_utf8_lossy(&buf).trim().to_string();
                    if line.is_empty() {
                        continue;
                    }
                    process_output_line(&app_out, &id_out, &procs_out, &line);
                }
                Err(_) => break,
            }
        }
    });

    // Read stderr (raw bytes, lossy decode)
    let app_err = app.clone();
    let id_err = task_id.clone();
    let procs_err = processes.clone();
    tokio::spawn(async move {
        let mut reader = tokio::io::BufReader::new(stderr);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match tokio::io::AsyncBufReadExt::read_until(&mut reader, b'\n', &mut buf).await {
                Ok(0) => break,
                Ok(_) => {
                    let line = String::from_utf8_lossy(&buf).trim().to_string();
                    if line.is_empty() {
                        continue;
                    }
                    process_output_line(&app_err, &id_err, &procs_err, &line);
                }
                Err(_) => break,
            }
        }
    });

    // Wait for process completion
    let app_wait = app.clone();
    let id_wait = task_id.clone();
    let procs_wait = processes.clone();
    tokio::spawn(async move {
        let status = child.wait().await;

        let was_cancelled = procs_wait
            .lock()
            .ok()
            .and_then(|map| map.get(&id_wait).map(|info| info.cancelled))
            .unwrap_or(false);

        let (output_file, has_output) = procs_wait
            .lock()
            .ok()
            .map(|map| {
                map.get(&id_wait)
                    .map(|info| {
                        (
                            info.output_files.last().cloned().unwrap_or_default(),
                            !info.output_files.is_empty(),
                        )
                    })
                    .unwrap_or_default()
            })
            .unwrap_or_default();

        let success = matches!(&status, Ok(s) if s.success());

        if success || has_output {
            // Treat as completed if exit code is 0 OR output files were produced
            let _ = app_wait.emit(
                "download-complete",
                serde_json::json!({ "id": id_wait, "outputFile": output_file }),
            );
        } else if !was_cancelled {
            let error_msg = status
                .as_ref()
                .map(|s| format!("进程退出码: {}", s.code().unwrap_or(-1)))
                .unwrap_or_else(|e| e.to_string());
            let _ = app_wait.emit(
                "download-error",
                serde_json::json!({
                    "id": id_wait,
                    "error": error_msg,
                }),
            );
        }

        // Clean up
        if let Ok(mut map) = procs_wait.lock() {
            map.remove(&id_wait);
        }
    });

    Ok(())
}

#[tauri::command]
pub async fn pause_download(
    state: tauri::State<'_, DownloadState>,
    id: String,
) -> Result<(), String> {
    let processes = state.processes.lock().map_err(|e| e.to_string())?;
    let info = processes.get(&id).ok_or("下载任务未找到")?;
    suspend_process_by_pid(info.pid)
}

#[tauri::command]
pub async fn resume_download(
    state: tauri::State<'_, DownloadState>,
    id: String,
) -> Result<(), String> {
    let processes = state.processes.lock().map_err(|e| e.to_string())?;
    let info = processes.get(&id).ok_or("下载任务未找到")?;
    resume_process_by_pid(info.pid)
}

#[tauri::command]
pub async fn cancel_download(
    state: tauri::State<'_, DownloadState>,
    id: String,
    delete_files: bool,
) -> Result<(), String> {
    let (pid, files) = {
        let mut processes = state.processes.lock().map_err(|e| e.to_string())?;
        let info = processes.get_mut(&id).ok_or("下载任务未找到")?;
        info.cancelled = true;
        (info.pid, info.output_files.clone())
    };

    kill_process_by_pid(pid)?;

    if delete_files {
        for file in &files {
            let _ = std::fs::remove_file(file);
            let _ = std::fs::remove_file(format!("{}.part", file));
        }
    }

    Ok(())
}
