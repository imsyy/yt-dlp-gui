//! 下载任务的暂停、恢复与取消。

use crate::platform::process;

use super::model::DownloadState;

/// 暂停下载任务（挂起子进程）
#[tauri::command]
pub async fn pause_download(
    state: tauri::State<'_, DownloadState>,
    id: String,
) -> Result<(), String> {
    let processes = state.processes.lock().map_err(|e| e.to_string())?;
    let info = processes.get(&id).ok_or("err_task_not_found")?;
    process::suspend_process(info.pid)
}

/// 继续下载任务（恢复子进程）
#[tauri::command]
pub async fn resume_download(
    state: tauri::State<'_, DownloadState>,
    id: String,
) -> Result<(), String> {
    let processes = state.processes.lock().map_err(|e| e.to_string())?;
    let info = processes.get(&id).ok_or("err_task_not_found")?;
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
        let info = processes.get_mut(&id).ok_or("err_task_not_found")?;
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
