//! yt-dlp/FFmpeg 输出解析与下载事件分发。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};

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
                info.errors.record(line);
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
                "fragmentIndex": info.fragment_index,
                "fragmentCount": info.fragment_count,
                "status": info.status,
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
                // 解析 ffmpeg 的 speed=Nx 字段（如 "speed=2.13x"）
                let ffmpeg_speed = parser::parse_ffmpeg_speed(line);
                let _ = app.emit(
                    "download-progress",
                    serde_json::json!({
                        "id": task_id,
                        "percent": percent,
                        "speed": ffmpeg_speed,
                        "eta": "",
                        "downloaded": format_duration(current_secs),
                        "total": format_duration(duration),
                        "status": "downloading",
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
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        read_output_lines(reader, |line| {
            process_output_line(&app, &task_id, &processes, line)
        })
        .await;
    })
}

/// Read whole pipe chunks without scheduling one async read for every byte.
/// Keep the line bound even if a tool writes arbitrarily long output.
async fn read_output_lines<R: tokio::io::AsyncRead + Unpin>(
    mut reader: R,
    mut process: impl FnMut(&str),
) {
    use tokio::io::AsyncReadExt;
    const MAX_LINE_LEN: usize = 64 * 1024;
    let mut line = Vec::with_capacity(1024);
    let mut chunk = [0u8; 8192];
    while let Ok(count) = reader.read(&mut chunk).await {
        if count == 0 {
            break;
        }
        for &byte in &chunk[..count] {
            if byte == b'\n' || byte == b'\r' {
                emit_output_line(&line, &mut process);
                line.clear();
            } else if line.len() < MAX_LINE_LEN {
                line.push(byte);
            }
        }
    }
    emit_output_line(&line, &mut process);
}

fn emit_output_line(line: &[u8], process: &mut impl FnMut(&str)) {
    if !line.is_empty() {
        let text = String::from_utf8_lossy(line);
        let text = text.trim();
        if !text.is_empty() {
            process(text);
        }
    }
}

#[cfg(test)]
mod line_reader_tests {
    use super::read_output_lines;

    fn read_lines(bytes: &[u8]) -> Vec<String> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let mut lines = Vec::new();
        runtime.block_on(read_output_lines(bytes, |line| {
            lines.push(line.to_string())
        }));
        lines
    }

    #[test]
    fn handles_ffmpeg_carriage_returns_crlf_and_unterminated_errors() {
        assert_eq!(
            read_lines(b"\r\n progress 1\rprogress 2\r\n\n  \nERROR: final error"),
            ["progress 1", "progress 2", "ERROR: final error"]
        );
    }

    #[test]
    fn preserves_unicode_across_chunk_boundaries() {
        let text = format!("{}中文🙂\nnext", "x".repeat(8191));
        assert_eq!(
            read_lines(text.as_bytes()),
            [text.split('\n').next().unwrap(), "next"]
        );
    }

    #[test]
    fn bounds_long_lines_without_losing_the_following_error() {
        let text = format!("{}\nERROR: retained\n", "x".repeat(100_000));
        let lines = read_lines(text.as_bytes());
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].len(), 64 * 1024);
        assert_eq!(lines[1], "ERROR: retained");
        assert_eq!(
            read_lines(b"invalid \xff\xfe\nvalid"),
            ["invalid ��", "valid"]
        );
    }
}

/// 启动异步任务等待子进程完成并发送结果事件
pub(super) fn spawn_completion_handler(
    app: AppHandle,
    task_id: String,
    processes: Arc<Mutex<HashMap<String, DownloadProcessInfo>>>,
    mut child: tokio::process::Child,
    output_readers: [tokio::task::JoinHandle<()>; 2],
) {
    tokio::spawn(async move {
        let status = child.wait().await;
        // The child can exit before buffered stderr/stdout has been consumed.
        // Drain both pipes before reading its final error or removing the task.
        drain_output_readers(output_readers, std::time::Duration::from_secs(5)).await;

        let was_cancelled = processes
            .lock()
            .ok()
            .and_then(|map| map.get(&task_id).map(|info| info.cancelled))
            .unwrap_or(false);

        // 仅以 yt-dlp 退出码判定成功；不能用「日志里见过 Destination 行」做兜底，
        // 因为 yt-dlp 在开始写字节前就会先打印目标路径，下载半路超时也会留下这一行。
        let success = matches!(&status, Ok(s) if s.success());

        // 取消优先于一切结果。用户取消后进程可能恰好以退出码 0 正常结束，若此处不先
        // 判断 was_cancelled，就会把已取消的任务覆盖成 completed，并弹出「下载完成」通知。
        // cancel_download 已把状态落为 cancelled，这里只需清理临时文件、不再写任何状态。
        if was_cancelled {
            let _ = resolve_output_file(&processes, &task_id);
        } else if success {
            let (output_file, _) = resolve_output_file(&processes, &task_id);
            let file_size_bytes = if !output_file.is_empty() {
                std::fs::metadata(&output_file)
                    .ok()
                    .map(|metadata| metadata.len())
            } else {
                None
            };

            // 真正直接由 Rust 后端写入 SQLite 完成状态与最终产物信息
            if let Some(db_state) = app.try_state::<crate::db::DatabaseState>() {
                let _ = crate::db::tasks::mark_task_completed(
                    &db_state,
                    &task_id,
                    &output_file,
                    file_size_bytes.map(|s| s as i64),
                );
            }

            let _ = app.emit(
                "download-complete",
                serde_json::json!({
                    "id": task_id,
                    "outputFile": output_file,
                    "fileSizeBytes": file_size_bytes,
                }),
            );
        } else {
            // 失败时仍清理 --print-to-file 临时文件，避免遗留
            let _ = resolve_output_file(&processes, &task_id);
            let error_msg = processes
                .lock()
                .ok()
                .and_then(|map| map.get(&task_id).and_then(|info| info.errors.summary()))
                .unwrap_or_else(|| {
                    status
                        .as_ref()
                        .map(|s| format!("err_exit_code:{}", s.code().unwrap_or(-1)))
                        .unwrap_or_else(|e| e.to_string())
                });

            // 真正直接由 Rust 后端写入 SQLite 错误状态与详情
            if let Some(db_state) = app.try_state::<crate::db::DatabaseState>() {
                let _ = crate::db::tasks::mark_task_error(&db_state, &task_id, &error_msg);
            }

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

/// A descendant can inherit a pipe after yt-dlp exits. Bound the shared drain
/// period so those inherited handles cannot hold a finished task's slot forever.
async fn drain_output_readers(
    mut readers: [tokio::task::JoinHandle<()>; 2],
    limit: std::time::Duration,
) {
    let mut drained = [false; 2];
    let _ = tokio::time::timeout(limit, async {
        for (index, reader) in readers.iter_mut().enumerate() {
            let _ = reader.await;
            drained[index] = true;
        }
    })
    .await;
    for (index, reader) in readers.iter().enumerate() {
        if !drained[index] {
            reader.abort();
        }
    }
    for (index, reader) in readers.into_iter().enumerate() {
        if !drained[index] {
            let _ = reader.await;
        }
    }
}

#[cfg(test)]
mod drain_tests {
    use super::{drain_output_readers, read_output_lines};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use tokio::io::AsyncWriteExt;

    #[test]
    fn inherited_pipe_does_not_block_completion_or_leave_a_reader_running() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let (reader, mut inherited_writer) = tokio::io::duplex(256);
            inherited_writer
                .write_all(b"ERROR: buffered error\n")
                .await
                .unwrap();
            let lines = Arc::new(Mutex::new(Vec::new()));
            let recorded = Arc::clone(&lines);
            let stuck = tokio::spawn(async move {
                read_output_lines(reader, |line| {
                    recorded.lock().unwrap().push(line.to_string())
                })
                .await;
            });
            let abort_stuck = stuck.abort_handle();
            let finished = tokio::spawn(async {});
            tokio::time::timeout(
                Duration::from_secs(2),
                drain_output_readers([stuck, finished], Duration::from_millis(30)),
            )
            .await
            .unwrap();
            assert!(abort_stuck.is_finished());
            assert_eq!(*lines.lock().unwrap(), ["ERROR: buffered error"]);
            // The inherited writer is deliberately kept alive past completion.
            assert!(inherited_writer.write_all(b"late output\n").await.is_err());
        });
    }

    #[test]
    fn normal_readers_are_fully_drained_before_returning() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let lines = Arc::new(Mutex::new(Vec::new()));
            let readers = ["stdout tail", "ERROR: stderr tail"].map(|text| {
                let recorded = Arc::clone(&lines);
                tokio::spawn(async move {
                    tokio::task::yield_now().await;
                    read_output_lines(text.as_bytes(), |line| {
                        recorded.lock().unwrap().push(line.to_string())
                    })
                    .await;
                })
            });
            drain_output_readers(readers, Duration::from_secs(2)).await;
            let mut result = lines.lock().unwrap().clone();
            result.sort();
            assert_eq!(result, ["ERROR: stderr tail", "stdout tail"]);
        });
    }
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
