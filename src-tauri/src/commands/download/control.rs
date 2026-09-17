//! 下载任务的取消与进程终止。

use crate::platform::process;

use super::model::DownloadState;

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
