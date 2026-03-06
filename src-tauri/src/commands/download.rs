/// 下载任务控制：启动、暂停、继续、取消、文件检查

use crate::parser;
use crate::process;
use crate::utils;
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};

use super::{DownloadParams, DownloadProcessInfo, DownloadState};

#[cfg(target_os = "windows")]
use super::CREATE_NO_WINDOW;

// ========== 输出处理 ==========

/// 处理 yt-dlp 的一行输出：解析进度并发送事件到前端
fn process_output_line(
    app: &AppHandle,
    task_id: &str,
    processes: &Arc<Mutex<HashMap<String, DownloadProcessInfo>>>,
    line: &str,
) {
    // 解析 --progress-template 输出的 JSON 进度
    if let Some(info) = parser::parse_progress_json(line) {
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
        return; // 进度行不需要转发到日志
    }

    // 跟踪输出文件路径（从 [download] Destination 等行解析，作为备选方案）
    if let Some(dest) = parse_destination(line) {
        if let Ok(mut map) = processes.lock() {
            if let Some(info) = map.get_mut(task_id) {
                info.output_files.push(dest);
            }
        }
    }

    // 转发日志到前端（不含进度 JSON 行，保持日志清晰）
    let _ = app.emit(
        "download-log",
        serde_json::json!({ "id": task_id, "line": line }),
    );
}

/// 从 yt-dlp 输出行中解析目标文件路径（备选方案，可能有编码问题）
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

/// 从临时文件中读取 yt-dlp --print-to-file 写出的最终文件路径
/// 返回最后一行（播放列表可能有多行）
fn read_filepath_from_file(filepath_file: &str) -> Option<String> {
    let content = std::fs::read_to_string(filepath_file).ok()?;
    let last_line = content.trim().lines().last()?.trim().to_string();
    if last_line.is_empty() {
        None
    } else {
        Some(last_line)
    }
}

// ========== 下载命令 ==========

/// 启动下载任务
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

    let args = build_download_args(&app, &params)?;

    // 生成临时文件路径，用于 --print-to-file 输出最终文件路径
    let app_data = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("获取应用数据目录失败: {}", e))?;
    let filepath_file = app_data
        .join(format!("{}_filepath.txt", params.id))
        .to_string_lossy()
        .to_string();

    // 拼接完整参数：基础参数 + --print-to-file
    let mut full_args = args;
    full_args.push("--print-to-file".to_string());
    full_args.push("after_move:filepath".to_string());
    full_args.push(filepath_file.clone());

    // 启动 yt-dlp 子进程
    let mut cmd = tokio::process::Command::new(&ytdlp_path);
    cmd.args(&full_args)
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

    // 记录进程信息
    let processes = state.processes.clone();
    {
        let mut map = processes.lock().map_err(|e| e.to_string())?;
        map.insert(
            task_id.clone(),
            DownloadProcessInfo {
                pid,
                cancelled: false,
                output_files: Vec::new(),
                download_dir: params.download_dir.clone(),
                filepath_file: Some(filepath_file),
            },
        );
    }

    let stdout = child.stdout.take().ok_or("获取 stdout 失败")?;
    let stderr = child.stderr.take().ok_or("获取 stderr 失败")?;

    // 读取 stdout（原始字节，lossy 解码以应对 Windows GBK 编码）
    spawn_output_reader(app.clone(), task_id.clone(), processes.clone(), stdout);
    // 读取 stderr
    spawn_output_reader(app.clone(), task_id.clone(), processes.clone(), stderr);

    // 等待进程完成并处理结果
    spawn_completion_handler(app.clone(), task_id, processes.clone(), child);

    Ok(())
}

/// 构建 yt-dlp 下载参数
fn build_download_args(app: &AppHandle, params: &DownloadParams) -> Result<Vec<String>, String> {
    let mut args: Vec<String> = vec![
        "--newline".to_string(),
        "--ignore-config".to_string(),  // 忽略用户系统配置，防止干扰 GUI
        "--color".to_string(), "never".to_string(),  // 禁用 ANSI 颜色转义序列
        // 使用 --progress-template 输出结构化进度（官方推荐方式，避免解析 stdout 常规输出）
        "--progress-template".to_string(),
        r#"download:PROGRESS_JSON:{"percent":"%(progress._percent_str|0%)s","speed":"%(progress._speed_str|)s","eta":"%(progress._eta_str|)s","downloaded":"%(progress._downloaded_bytes_str|)s","total":"%(progress._total_bytes_str|)s"}"#.to_string(),
    ];

    // JS 运行时（Deno）
    args.extend(utils::build_js_runtime_args(app));

    // 格式选择
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
            let vf = params.video_format.as_deref().filter(|s| !s.is_empty());
            let af = params.audio_format.as_deref().filter(|s| !s.is_empty());
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

    // 输出路径模板
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

    // 额外选项
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

    // 字幕
    if !params.subtitles.is_empty() {
        args.push("--write-subs".to_string());
        args.push("--sub-langs".to_string());
        args.push(params.subtitles.join(","));
    }

    // 时间范围裁剪（仅在有实际裁剪范围时添加，避免 *0-inf 触发不必要的 ffmpeg 处理）
    let has_start = params.start_time.is_some_and(|t| t > 0.0);
    let has_end = params.end_time.is_some();
    if has_start || has_end {
        let start = params.start_time.unwrap_or(0.0);
        let end_str = params
            .end_time
            .map(|t| format!("{}", t))
            .unwrap_or_else(|| "inf".to_string());
        args.push("--download-sections".to_string());
        args.push(format!("*{}-{}", start, end_str));
    }

    // 播放列表
    if params.no_playlist {
        args.push("--no-playlist".to_string());
    } else if let Some(ref items) = params.playlist_items {
        if !items.is_empty() {
            args.push("--playlist-items".to_string());
            args.push(items.clone());
        }
    }

    // URL（必须放在最后）
    args.push(params.url.clone());

    Ok(args)
}

/// 启动异步任务读取子进程输出流
fn spawn_output_reader<R: tokio::io::AsyncRead + Unpin + Send + 'static>(
    app: AppHandle,
    task_id: String,
    processes: Arc<Mutex<HashMap<String, DownloadProcessInfo>>>,
    reader: R,
) {
    tokio::spawn(async move {
        let mut buf_reader = tokio::io::BufReader::new(reader);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match tokio::io::AsyncBufReadExt::read_until(&mut buf_reader, b'\n', &mut buf).await {
                Ok(0) => break,
                Ok(_) => {
                    let line = String::from_utf8_lossy(&buf).trim().to_string();
                    if line.is_empty() {
                        continue;
                    }
                    process_output_line(&app, &task_id, &processes, &line);
                }
                Err(_) => break,
            }
        }
    });
}

/// 启动异步任务等待子进程完成并发送结果事件
fn spawn_completion_handler(
    app: AppHandle,
    task_id: String,
    processes: Arc<Mutex<HashMap<String, DownloadProcessInfo>>>,
    mut child: tokio::process::Child,
) {
    tokio::spawn(async move {
        let status = child.wait().await;

        let was_cancelled = processes
            .lock()
            .ok()
            .and_then(|map| map.get(&task_id).map(|info| info.cancelled))
            .unwrap_or(false);

        // 获取最终输出文件路径
        let (output_file, has_output) = resolve_output_file(&processes, &task_id);

        let success = matches!(&status, Ok(s) if s.success());

        if success || has_output {
            let _ = app.emit(
                "download-complete",
                serde_json::json!({ "id": task_id, "outputFile": output_file }),
            );
        } else if !was_cancelled {
            let error_msg = status
                .as_ref()
                .map(|s| format!("进程退出码: {}", s.code().unwrap_or(-1)))
                .unwrap_or_else(|e| e.to_string());
            let _ = app.emit(
                "download-error",
                serde_json::json!({
                    "id": task_id,
                    "error": error_msg,
                }),
            );
        }

        // 清理进程记录
        if let Ok(mut map) = processes.lock() {
            map.remove(&task_id);
        }
    });
}

/// 解析最终输出文件路径
/// 优先从 --print-to-file 临时文件读取（UTF-8 可靠），回退到 stdout 解析结果
fn resolve_output_file(
    processes: &Arc<Mutex<HashMap<String, DownloadProcessInfo>>>,
    task_id: &str,
) -> (String, bool) {
    processes
        .lock()
        .ok()
        .map(|map| {
            map.get(task_id)
                .map(|info| {
                    let mut file = String::new();

                    // 优先从临时文件读取（避免 Windows stdout GBK 编码乱码问题）
                    if let Some(ref fp_file) = info.filepath_file {
                        if let Some(path) = read_filepath_from_file(fp_file) {
                            file = path;
                        }
                        // 清理临时文件
                        let _ = std::fs::remove_file(fp_file);
                    }

                    // 回退：从 stdout 解析的路径
                    if file.is_empty() {
                        file = info.output_files.last().cloned().unwrap_or_default();
                        // 相对路径补全为绝对路径
                        if !file.is_empty() && !std::path::Path::new(&file).is_absolute() {
                            file = std::path::PathBuf::from(&info.download_dir)
                                .join(&file)
                                .to_string_lossy()
                                .to_string();
                        }
                    }

                    let has = !info.output_files.is_empty() || !file.is_empty();
                    (file, has)
                })
                .unwrap_or_default()
        })
        .unwrap_or_default()
}

// ========== 下载控制命令 ==========

/// 暂停下载任务（挂起子进程）
#[tauri::command]
pub async fn pause_download(
    state: tauri::State<'_, DownloadState>,
    id: String,
) -> Result<(), String> {
    let processes = state.processes.lock().map_err(|e| e.to_string())?;
    let info = processes.get(&id).ok_or("下载任务未找到")?;
    process::suspend_process(info.pid)
}

/// 继续下载任务（恢复子进程）
#[tauri::command]
pub async fn resume_download(
    state: tauri::State<'_, DownloadState>,
    id: String,
) -> Result<(), String> {
    let processes = state.processes.lock().map_err(|e| e.to_string())?;
    let info = processes.get(&id).ok_or("下载任务未找到")?;
    process::resume_process(info.pid)
}

/// 取消下载任务并可选删除已下载文件
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

    process::kill_process(pid)?;

    if delete_files {
        for file in &files {
            let _ = std::fs::remove_file(file);
            let _ = std::fs::remove_file(format!("{}.part", file));
        }
    }

    Ok(())
}

// ========== 文件检查 ==========

/// 批量检查文件是否存在
#[tauri::command]
pub fn check_files_exist(paths: Vec<String>) -> Vec<bool> {
    paths
        .iter()
        .map(|p| std::path::Path::new(p).exists())
        .collect()
}

/// 删除指定文件
#[tauri::command]
pub fn delete_file(path: String) -> Result<(), String> {
    let p = std::path::Path::new(&path);
    if p.exists() {
        std::fs::remove_file(p).map_err(|e| format!("删除文件失败: {}", e))?;
    }
    Ok(())
}
