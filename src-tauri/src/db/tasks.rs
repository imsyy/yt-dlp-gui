//! 下载任务数据模型、持久化与生命周期状态更新。

use rusqlite::{params, Connection};
use tauri::State;

use super::DatabaseState;

/// 前后端交互的下载任务数据结构
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DownloadTaskRecord {
    pub id: String,
    pub url: String,
    pub title: String,
    pub thumbnail: String,
    pub format_label: String,
    pub status: String,
    pub percent: f64,
    pub speed: String,
    pub eta: String,
    pub downloaded: String,
    pub total: String,
    pub file_size_bytes: Option<i64>,
    pub logs: Vec<String>,
    pub error: Option<String>,
    pub output_file: Option<String>,
    pub created_at: i64,
    pub params: serde_json::Value,
}

// 内部数据库读写辅助函数

pub fn get_all_tasks(conn: &Connection) -> Result<Vec<DownloadTaskRecord>, rusqlite::Error> {
    // 状态归一化：将非正常退出前处于进行中或暂停态的任务自动置为错误中断态
    let _ = conn.execute(
        "UPDATE tasks SET status = 'error', error = '应用已重启，任务已中断', speed = '', eta = '' \
         WHERE status IN ('preparing', 'queued', 'downloading', 'postprocessing', 'paused')",
        [],
    );

    let mut stmt = conn.prepare(
        "SELECT id, url, title, thumbnail, format_label, status, percent, speed, eta, \
         downloaded, total, file_size_bytes, logs, error, output_file, created_at, params_json \
         FROM tasks ORDER BY created_at DESC",
    )?;

    let rows = stmt.query_map([], |row| {
        let id: String = row.get(0)?;
        let url: String = row.get(1)?;
        let title: String = row.get(2)?;
        let thumbnail: String = row.get(3)?;
        let format_label: String = row.get(4)?;
        let status: String = row.get(5)?;
        let percent: f64 = row.get(6)?;
        let speed: String = row.get(7)?;
        let eta: String = row.get(8)?;
        let downloaded: String = row.get(9)?;
        let total: String = row.get(10)?;
        let file_size_bytes: Option<i64> = row.get(11)?;
        let logs_raw: String = row.get(12)?;
        let error: Option<String> = row.get(13)?;
        let output_file: Option<String> = row.get(14)?;
        let created_at: i64 = row.get(15)?;
        let params_raw: String = row.get(16)?;

        let logs: Vec<String> = serde_json::from_str(&logs_raw).unwrap_or_default();
        let params: serde_json::Value = serde_json::from_str(&params_raw)
            .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

        Ok(DownloadTaskRecord {
            id,
            url,
            title,
            thumbnail,
            format_label,
            status,
            percent,
            speed,
            eta,
            downloaded,
            total,
            file_size_bytes,
            logs,
            error,
            output_file,
            created_at,
            params,
        })
    })?;

    let mut tasks = Vec::new();
    for task_result in rows {
        tasks.push(task_result?);
    }
    Ok(tasks)
}

pub fn upsert_task_internal(conn: &Connection, task: &DownloadTaskRecord) -> Result<(), rusqlite::Error> {
    let logs_json = serde_json::to_string(&task.logs).unwrap_or_else(|_| "[]".to_string());
    let params_json = serde_json::to_string(&task.params).unwrap_or_else(|_| "{}".to_string());

    conn.execute(
        r#"
        INSERT INTO tasks (
            id, url, title, thumbnail, format_label, status, percent, speed, eta,
            downloaded, total, file_size_bytes, logs, error, output_file, created_at, params_json
        ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17
        ) ON CONFLICT(id) DO UPDATE SET
            url = excluded.url,
            title = excluded.title,
            thumbnail = excluded.thumbnail,
            format_label = excluded.format_label,
            status = excluded.status,
            percent = excluded.percent,
            speed = excluded.speed,
            eta = excluded.eta,
            downloaded = excluded.downloaded,
            total = excluded.total,
            file_size_bytes = excluded.file_size_bytes,
            logs = excluded.logs,
            error = excluded.error,
            output_file = excluded.output_file,
            created_at = excluded.created_at,
            params_json = excluded.params_json
        "#,
        params![
            task.id,
            task.url,
            task.title,
            task.thumbnail,
            task.format_label,
            task.status,
            task.percent,
            task.speed,
            task.eta,
            task.downloaded,
            task.total,
            task.file_size_bytes,
            logs_json,
            task.error,
            task.output_file,
            task.created_at,
            params_json,
        ],
    )?;

    Ok(())
}

/// 后端生命周期调用：标记任务为下载中
pub fn mark_task_downloading(state: &DatabaseState, id: &str) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE tasks SET status = 'downloading', error = NULL WHERE id = ?1",
        params![id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 后端生命周期调用：标记任务已完成并更新产物信息
pub fn mark_task_completed(
    state: &DatabaseState,
    id: &str,
    output_file: &str,
    file_size_bytes: Option<i64>,
) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE tasks SET status = 'completed', percent = 100, speed = '', eta = '', \
         output_file = ?2, file_size_bytes = ?3, error = NULL WHERE id = ?1",
        params![id, output_file, file_size_bytes],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 后端生命周期调用：标记任务失败并记录错误信息
pub fn mark_task_error(state: &DatabaseState, id: &str, error_msg: &str) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE tasks SET status = 'error', speed = '', eta = '', error = ?2 WHERE id = ?1",
        params![id, error_msg],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 后端生命周期调用：标记任务已被用户取消
pub fn mark_task_cancelled(state: &DatabaseState, id: &str) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE tasks SET status = 'cancelled', speed = '', eta = '' WHERE id = ?1",
        params![id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

// 暴露给前端调用的 Tauri 命令

/// 获取所有持久化任务列表
#[tauri::command]
pub fn db_get_tasks(state: State<'_, DatabaseState>) -> Result<Vec<DownloadTaskRecord>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    get_all_tasks(&conn).map_err(|e| e.to_string())
}

/// 创建或更新单个任务
#[tauri::command]
pub fn db_upsert_task(
    state: State<'_, DatabaseState>,
    task: DownloadTaskRecord,
) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    upsert_task_internal(&conn, &task).map_err(|e| e.to_string())
}

/// 批量创建或更新任务（常用于老版本任务数据平滑迁移）
pub fn upsert_tasks_batch(
    conn: &mut Connection,
    tasks: &[DownloadTaskRecord],
) -> Result<(), rusqlite::Error> {
    if tasks.is_empty() {
        return Ok(());
    }
    let tx = conn.transaction()?;
    for task in tasks {
        upsert_task_internal(&tx, task)?;
    }
    tx.commit()?;
    Ok(())
}

/// 批量创建或更新任务命令（常用于老版本任务数据平滑迁移）
#[tauri::command]
pub fn db_upsert_tasks_batch(
    state: State<'_, DatabaseState>,
    tasks: Vec<DownloadTaskRecord>,
) -> Result<(), String> {
    let mut conn = state.conn.lock().map_err(|e| e.to_string())?;
    upsert_tasks_batch(&mut conn, &tasks).map_err(|e| e.to_string())
}

/// 删除单个任务
#[tauri::command]
pub fn db_delete_task(state: State<'_, DatabaseState>, id: String) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM tasks WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// 批量删除指定 ID 的任务
#[tauri::command]
pub fn db_delete_tasks(state: State<'_, DatabaseState>, ids: Vec<String>) -> Result<(), String> {
    if ids.is_empty() {
        return Ok(());
    }
    let mut conn = state.conn.lock().map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    for id in ids {
        tx.execute("DELETE FROM tasks WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

/// 清除所有已完成的任务
#[tauri::command]
pub fn db_clear_completed_tasks(state: State<'_, DatabaseState>) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM tasks WHERE status = 'completed'", [])
        .map_err(|e| e.to_string())?;
    Ok(())
}
