//! 工具页当前结果快照。每个工具只保留一条，不构成历史记录。

use super::DatabaseState;
use rusqlite::{params, OptionalExtension};
use tauri::State;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolSnapshotRecord {
    pub tool: String,
    pub url: String,
    pub title: String,
    pub result_json: String,
    pub updated_at: i64,
}

#[tauri::command]
pub fn db_save_tool_snapshot(
    state: State<'_, DatabaseState>,
    tool: String,
    url: String,
    title: String,
    result_json: String,
) -> Result<(), String> {
    let tool = tool.trim();
    let url = url.trim();
    if tool.is_empty() || url.is_empty() {
        return Err("tool and url must not be empty".into());
    }
    let updated_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or_default();
    let conn = state.conn.lock().map_err(|error| error.to_string())?;
    conn.execute(
        "INSERT INTO tool_snapshots (tool, url, title, result_json, updated_at) VALUES (?1, ?2, ?3, ?4, ?5) ON CONFLICT(tool) DO UPDATE SET url = excluded.url, title = excluded.title, result_json = excluded.result_json, updated_at = excluded.updated_at",
        params![tool, url, title, result_json, updated_at],
    ).map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn db_get_tool_snapshot(
    state: State<'_, DatabaseState>,
    tool: String,
) -> Result<Option<ToolSnapshotRecord>, String> {
    let conn = state.conn.lock().map_err(|error| error.to_string())?;
    conn.query_row(
        "SELECT tool, url, title, result_json, updated_at FROM tool_snapshots WHERE tool = ?1",
        params![tool.trim()],
        |row| Ok(ToolSnapshotRecord { tool: row.get(0)?, url: row.get(1)?, title: row.get(2)?, result_json: row.get(3)?, updated_at: row.get(4)? }),
    ).optional().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn db_clear_tool_snapshot(
    state: State<'_, DatabaseState>,
    tool: String,
) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|error| error.to_string())?;
    conn.execute("DELETE FROM tool_snapshots WHERE tool = ?1", params![tool.trim()])
        .map_err(|error| error.to_string())?;
    Ok(())
}
