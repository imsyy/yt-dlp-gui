//! URL 解析历史数据模型与增删查改。

use rusqlite::{params, Connection};
use tauri::State;

use super::DatabaseState;

/// 前后端交互的解析历史数据结构
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct HistoryItemRecord {
    pub url: String,
    pub title: String,
    pub time: i64,
}

// 内部数据库读写辅助函数

pub fn get_recent_history(conn: &Connection) -> Result<Vec<HistoryItemRecord>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT url, title, accessed_at FROM parse_history ORDER BY accessed_at DESC LIMIT 50",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(HistoryItemRecord {
            url: row.get(0)?,
            title: row.get(1)?,
            time: row.get(2)?,
        })
    })?;

    let mut history = Vec::new();
    for item in rows {
        history.push(item?);
    }
    Ok(history)
}

pub fn add_history_entry(conn: &Connection, url: &str, title: &str) -> Result<(), rusqlite::Error> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);

    conn.execute(
        r#"
        INSERT INTO parse_history (url, title, accessed_at) VALUES (?1, ?2, ?3)
        ON CONFLICT(url) DO UPDATE SET title = excluded.title, accessed_at = excluded.accessed_at
        "#,
        params![url, title, now],
    )?;

    // 自动裁剪超出 50 条的旧记录
    let _ = conn.execute(
        "DELETE FROM parse_history WHERE id NOT IN (SELECT id FROM parse_history ORDER BY accessed_at DESC LIMIT 50)",
        [],
    );

    Ok(())
}

pub fn add_history_batch(
    conn: &mut Connection,
    items: &[HistoryItemRecord],
) -> Result<(), rusqlite::Error> {
    if items.is_empty() {
        return Ok(());
    }
    let tx = conn.transaction()?;
    {
        let mut stmt = tx.prepare(
            r#"
            INSERT INTO parse_history (url, title, accessed_at) VALUES (?1, ?2, ?3)
            ON CONFLICT(url) DO UPDATE SET title = excluded.title, accessed_at = excluded.accessed_at
            "#,
        )?;

        for item in items {
            let trimmed = item.url.trim();
            if trimmed.is_empty() {
                continue;
            }
            let title = if item.title.trim().is_empty() {
                trimmed
            } else {
                item.title.trim()
            };
            let time = if item.time > 0 {
                item.time
            } else {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as i64)
                    .unwrap_or(0)
            };
            stmt.execute(params![trimmed, title, time])?;
        }
    }

    // 自动裁剪超出 50 条的旧记录
    tx.execute(
        "DELETE FROM parse_history WHERE id NOT IN (SELECT id FROM parse_history ORDER BY accessed_at DESC LIMIT 50)",
        [],
    )?;

    tx.commit()?;
    Ok(())
}

// 暴露给前端调用的 Tauri 命令

/// 获取最近 50 条解析历史
#[tauri::command]
pub fn db_get_history(state: State<'_, DatabaseState>) -> Result<Vec<HistoryItemRecord>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    get_recent_history(&conn).map_err(|e| e.to_string())
}

/// 新增或更新一条解析历史（自动保留最新 50 条）
#[tauri::command]
pub fn db_add_history(
    state: State<'_, DatabaseState>,
    url: String,
    title: String,
) -> Result<(), String> {
    let trimmed_url = url.trim();
    if trimmed_url.is_empty() {
        return Ok(());
    }
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    add_history_entry(&conn, trimmed_url, &title).map_err(|e| e.to_string())
}

/// 批量新增或更新解析历史（常用于老版本历史数据平滑迁移）
#[tauri::command]
pub fn db_add_history_batch(
    state: State<'_, DatabaseState>,
    items: Vec<HistoryItemRecord>,
) -> Result<(), String> {
    let mut conn = state.conn.lock().map_err(|e| e.to_string())?;
    add_history_batch(&mut conn, &items).map_err(|e| e.to_string())
}

/// 删除单条历史
#[tauri::command]
pub fn db_remove_history(state: State<'_, DatabaseState>, url: String) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM parse_history WHERE url = ?1", params![url.trim()])
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// 清空历史
#[tauri::command]
pub fn db_clear_history(state: State<'_, DatabaseState>) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM parse_history", [])
        .map_err(|e| e.to_string())?;
    Ok(())
}
