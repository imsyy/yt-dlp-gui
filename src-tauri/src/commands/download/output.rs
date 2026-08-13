//! yt-dlp/FFmpeg 输出解析与下载事件分发。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

use super::model::DownloadProcessInfo;
use super::parser;

/// 将秒数格式化为 HH:MM:SS
fn format_duration(secs: f64) -> String {
    let total = secs as u64;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    if h > 0 {
        format!("{:02}:{:02}:{:02}", h, m, s)
    } else {
        format!("{:02}:{:02}", m, s)
    }
}

/// 处理 yt-dlp 的一行输出：解析进度并发送事件到前端
fn process_output_line(
    app: &AppHandle,
    task_id: &str,
    processes: &Arc<Mutex<HashMap<String, DownloadProcessInfo>>>,
    line: &str,
) {
    if line.starts_with("ERROR:") {
        if let Ok(mut map) = processes.lock() {
            if let Some(info) = map.get_mut(task_id) {
                info.last_error = Some(line.to_string());
            }
        }
    }

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

    // 解析 ffmpeg 输出中的 time= 字段（用于时间裁剪场景的进度）
    if line.contains("time=") && line.contains("frame=") {
        if let Some(current_secs) = parser::parse_ffmpeg_time(line) {
            let clip_dur = processes
                .lock()
                .ok()
                .and_then(|map| map.get(task_id).and_then(|info| info.clip_duration));
            if let Some(duration) = clip_dur {
                let percent = (current_secs / duration * 100.0).min(100.0);
                let _ = app.emit(
                    "download-progress",
                    serde_json::json!({
                        "id": task_id,
                        "percent": percent,
                        "speed": "",
                        "eta": "",
                        "downloaded": format_duration(current_secs),
                        "total": format_duration(duration),
                    }),
                );
            }
        }
        return; // ffmpeg 帧进度不转发到日志
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

/// 启动异步任务读取子进程输出流
/// 同时处理 \n 和 \r 作为行分隔符（ffmpeg 进度输出使用 \r）
pub(super) fn spawn_output_reader<R: tokio::io::AsyncRead + Unpin + Send + 'static>(
    app: AppHandle,
    task_id: String,
    processes: Arc<Mutex<HashMap<String, DownloadProcessInfo>>>,
    reader: R,
) {
    tokio::spawn(async move {
        use tokio::io::AsyncReadExt;
        let mut buf_reader = tokio::io::BufReader::new(reader);
        const MAX_LINE_LEN: usize = 64 * 1024; // 64KB
        let mut line_buf = Vec::with_capacity(1024);
        let mut byte_buf = [0u8; 1];

        loop {
            match buf_reader.read(&mut byte_buf).await {
                Ok(0) => {
                    // EOF：处理缓冲区中剩余的内容
                    if !line_buf.is_empty() {
                        let line = String::from_utf8_lossy(&line_buf).trim().to_string();
                        if !line.is_empty() {
                            process_output_line(&app, &task_id, &processes, &line);
                        }
                    }
                    break;
                }
                Ok(_) => {
                    if byte_buf[0] == b'\n' || byte_buf[0] == b'\r' {
                        if !line_buf.is_empty() {
                            let line = String::from_utf8_lossy(&line_buf).trim().to_string();
                            if !line.is_empty() {
                                process_output_line(&app, &task_id, &processes, &line);
                            }
                            line_buf.clear();
                        }
                    } else if line_buf.len() < MAX_LINE_LEN {
                        line_buf.push(byte_buf[0]);
                    }
                }
                Err(_) => break,
            }
        }
    });
}

/// 启动异步任务等待子进程完成并发送结果事件
pub(super) fn spawn_completion_handler(
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

        // 仅以 yt-dlp 退出码判定成功；不能用「日志里见过 Destination 行」做兜底，
        // 因为 yt-dlp 在开始写字节前就会先打印目标路径，下载半路超时也会留下这一行。
        let success = matches!(&status, Ok(s) if s.success());

        if success {
            let (output_file, _) = resolve_output_file(&processes, &task_id);
            let _ = app.emit(
                "download-complete",
                serde_json::json!({ "id": task_id, "outputFile": output_file }),
            );
        } else if !was_cancelled {
            // 失败时仍清理 --print-to-file 临时文件，避免遗留
            let _ = resolve_output_file(&processes, &task_id);
            let error_msg = processes
                .lock()
                .ok()
                .and_then(|map| map.get(&task_id).and_then(|info| info.last_error.clone()))
                .unwrap_or_else(|| {
                    status
                        .as_ref()
                        .map(|s| format!("err_exit_code:{}", s.code().unwrap_or(-1)))
                        .unwrap_or_else(|e| e.to_string())
                });
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
