//! 下载子进程的创建与生命周期启动。

use crate::{commands::CREATE_NO_WINDOW, utils};
use std::process::Stdio;
use tauri::{AppHandle, Manager};

use super::{
    arguments::{build_download_args, requires_ffmpeg_merge},
    model::{DownloadParams, DownloadProcessInfo, DownloadState},
    output::{spawn_completion_handler, spawn_output_reader},
};

/// 启动下载任务
#[tauri::command]
pub async fn start_download(
    app: AppHandle,
    state: tauri::State<'_, DownloadState>,
    params: DownloadParams,
) -> Result<(), String> {
    let ytdlp_path = utils::get_ytdlp_path(&app)?;
    if !ytdlp_path.exists() {
        return Err("err_ytdlp_not_installed".to_string());
    }
    if requires_ffmpeg_merge(&params)
        && (!utils::get_ffmpeg_path(&app)?.exists() || !utils::get_ffprobe_path(&app)?.exists())
    {
        return Err("err_ffmpeg_required_for_merge".to_string());
    }

    let args = build_download_args(&app, &params)?;

    // 生成临时文件路径，用于 --print-to-file 输出最终文件路径
    let app_data = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("err_app_data_dir:{}", e))?;
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
        .map_err(|e| format!("err_start_download:{}", e))?;

    let pid = child.id().ok_or("err_get_pid")?;
    let task_id = params.id.clone();

    // 计算裁剪片段时长（用于 ffmpeg 进度计算）
    let clip_duration = match (params.start_time, params.end_time) {
        (Some(s), Some(e)) => Some(e - s),
        (None, Some(e)) => Some(e),
        _ => None,
    };

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
                clip_duration,
                last_error: None,
            },
        );
    }

    let stdout = child.stdout.take().ok_or("err_capture_stdout")?;
    let stderr = child.stderr.take().ok_or("err_capture_stderr")?;

    // 读取 stdout（原始字节，lossy 解码以应对 Windows GBK 编码）
    spawn_output_reader(app.clone(), task_id.clone(), processes.clone(), stdout);
    // 读取 stderr
    spawn_output_reader(app.clone(), task_id.clone(), processes.clone(), stderr);

    // 等待进程完成并处理结果
    spawn_completion_handler(app.clone(), task_id, processes.clone(), child);

    Ok(())
}
