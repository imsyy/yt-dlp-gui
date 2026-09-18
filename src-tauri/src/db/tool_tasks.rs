//! 工具后台任务状态和最后成功结果元数据。

use super::DatabaseState;
use rusqlite::{params, OptionalExtension};

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolTaskStateRecord {
    pub tool_id: String,
    pub run_id: String,
    pub status: String,
    pub url: String,
    pub stage: Option<String>,
    pub progress: Option<f64>,
    pub error: Option<String>,
    pub started_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolResultRecord {
    pub tool_id: String,
    pub run_id: String,
    pub result_type: String,
    pub result_json: Option<String>,
    pub total: i64,
    pub completed_at: i64,
}

pub fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or_default()
}

pub fn get_state(db: &DatabaseState, tool_id: &str) -> Result<Option<ToolTaskStateRecord>, String> {
    let conn = db.conn.lock().map_err(|error| error.to_string())?;
    conn.query_row(
        "SELECT tool_id, run_id, status, url, stage, progress, error, started_at, updated_at FROM tool_task_states WHERE tool_id = ?1",
        params![tool_id],
        |row| Ok(ToolTaskStateRecord { tool_id: row.get(0)?, run_id: row.get(1)?, status: row.get(2)?, url: row.get(3)?, stage: row.get(4)?, progress: row.get(5)?, error: row.get(6)?, started_at: row.get(7)?, updated_at: row.get(8)? }),
    ).optional().map_err(|error| error.to_string())
}

/// 正在运行的工具 id 列表，供状态栏全局指示使用。
pub fn get_running_tool_ids(db: &DatabaseState) -> Result<Vec<String>, String> {
    let conn = db.conn.lock().map_err(|error| error.to_string())?;
    let mut stmt = conn
        .prepare("SELECT tool_id FROM tool_task_states WHERE status = 'running' ORDER BY tool_id")
        .map_err(|error| error.to_string())?;
    let ids = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|error| error.to_string())?
        .filter_map(Result::ok)
        .collect();
    Ok(ids)
}

pub fn start_state(
    db: &DatabaseState,
    tool_id: &str,
    run_id: &str,
    url: &str,
) -> Result<ToolTaskStateRecord, String> {
    if get_state(db, tool_id)?.is_some_and(|state| state.status == "running") {
        return Err("err_tool_already_running".into());
    }
    let now = now_millis();
    let conn = db.conn.lock().map_err(|error| error.to_string())?;
    conn.execute(
        "INSERT INTO tool_task_states (tool_id, run_id, status, url, stage, progress, error, started_at, updated_at) VALUES (?1, ?2, 'running', ?3, 'preparing', 0, NULL, ?4, ?4) ON CONFLICT(tool_id) DO UPDATE SET run_id = excluded.run_id, status = 'running', url = excluded.url, stage = 'preparing', progress = 0, error = NULL, started_at = excluded.started_at, updated_at = excluded.updated_at",
        params![tool_id, run_id, url, now],
    ).map_err(|error| error.to_string())?;
    drop(conn);
    get_state(db, tool_id)?.ok_or_else(|| "err_tool_state_missing".into())
}

/// 写入终态。仅在状态仍为 running 时生效；已被取消时视为无操作并返回 None。
pub fn finish_state(
    db: &DatabaseState,
    tool_id: &str,
    run_id: &str,
    status: &str,
    error: Option<&str>,
) -> Result<Option<ToolTaskStateRecord>, String> {
    let now = now_millis();
    let conn = db.conn.lock().map_err(|error| error.to_string())?;
    let affected = conn.execute(
        "UPDATE tool_task_states SET status = ?3, stage = NULL, progress = CASE WHEN ?3 = 'completed' THEN 100 ELSE progress END, error = ?4, updated_at = ?5 WHERE tool_id = ?1 AND run_id = ?2 AND status = 'running'",
        params![tool_id, run_id, status, error, now],
    ).map_err(|error| error.to_string())?;
    drop(conn);
    if affected == 0 {
        return Ok(None);
    }
    get_state(db, tool_id)
}

/// 更新阶段。仅在状态仍为 running 时生效，否则返回 None（与 `finish_state` 同语义）。
pub fn update_running_stage(
    db: &DatabaseState,
    tool_id: &str,
    run_id: &str,
    stage: &str,
) -> Result<Option<ToolTaskStateRecord>, String> {
    let conn = db.conn.lock().map_err(|error| error.to_string())?;
    let affected = conn
        .execute(
            "UPDATE tool_task_states SET stage = ?3, updated_at = ?4 WHERE tool_id = ?1 AND run_id = ?2 AND status = 'running'",
            params![tool_id, run_id, stage, now_millis()],
        )
        .map_err(|error| error.to_string())?;
    drop(conn);
    if affected == 0 {
        return Ok(None);
    }
    get_state(db, tool_id)
}

pub fn save_json_result(
    db: &DatabaseState,
    tool_id: &str,
    run_id: &str,
    result_json: &str,
    total: i64,
) -> Result<(), String> {
    let conn = db.conn.lock().map_err(|error| error.to_string())?;
    conn.execute(
        "INSERT INTO tool_results (tool_id, run_id, result_type, result_json, result_file, total, completed_at) VALUES (?1, ?2, 'json', ?3, NULL, ?4, ?5) ON CONFLICT(tool_id) DO UPDATE SET run_id = excluded.run_id, result_type = 'json', result_json = excluded.result_json, result_file = NULL, total = excluded.total, completed_at = excluded.completed_at",
        params![tool_id, run_id, result_json, total, now_millis()],
    ).map_err(|error| error.to_string())?;
    Ok(())
}

pub fn save_file_result(
    db: &DatabaseState,
    tool_id: &str,
    run_id: &str,
    relative_file: &str,
    total: i64,
) -> Result<Option<String>, String> {
    let conn = db.conn.lock().map_err(|error| error.to_string())?;
    let previous = conn
        .query_row(
            "SELECT result_file FROM tool_results WHERE tool_id = ?1",
            params![tool_id],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?
        .flatten();
    conn.execute(
        "INSERT INTO tool_results (tool_id, run_id, result_type, result_json, result_file, total, completed_at) VALUES (?1, ?2, 'paged', NULL, ?3, ?4, ?5) ON CONFLICT(tool_id) DO UPDATE SET run_id = excluded.run_id, result_type = 'paged', result_json = NULL, result_file = excluded.result_file, total = excluded.total, completed_at = excluded.completed_at",
        params![tool_id, run_id, relative_file, total, now_millis()],
    ).map_err(|error| error.to_string())?;
    Ok(previous)
}

pub fn get_result(
    db: &DatabaseState,
    tool_id: &str,
) -> Result<Option<(ToolResultRecord, Option<String>)>, String> {
    let conn = db.conn.lock().map_err(|error| error.to_string())?;
    conn.query_row(
        "SELECT tool_id, run_id, result_type, result_json, result_file, total, completed_at FROM tool_results WHERE tool_id = ?1",
        params![tool_id],
        |row| Ok((ToolResultRecord { tool_id: row.get(0)?, run_id: row.get(1)?, result_type: row.get(2)?, result_json: row.get(3)?, total: row.get(5)?, completed_at: row.get(6)? }, row.get(4)?)),
    ).optional().map_err(|error| error.to_string())
}
