//! 下载任务的取消与进程终止。

use crate::db::tasks::mark_task_cancelled;
use crate::db::DatabaseState;
use crate::platform::process;

use super::model::DownloadState;

/// 取消下载任务并可选删除已下载文件
#[tauri::command]
pub async fn cancel_download(
    state: tauri::State<'_, DownloadState>,
    db_state: tauri::State<'_, DatabaseState>,
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

    // 真正直接由 Rust 后端写入 SQLite 取消状态，无需前端调度
    let _ = mark_task_cancelled(&db_state, &id);

    if delete_files {
        for file in &files {
            let _ = std::fs::remove_file(file);
            let _ = std::fs::remove_file(format!("{}.part", file));
        }
    }

    Ok(())
}
