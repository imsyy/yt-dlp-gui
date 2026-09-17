//! 本地 SQLite 嵌入式数据库核心管理模块。

pub mod history;
pub mod schema;
pub mod tasks;

use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Manager, State};

/// 托管在 Tauri 状态树中的 SQLite 数据库连接实例
pub struct DatabaseState {
    pub conn: Mutex<Connection>,
    pub db_path: PathBuf,
}

/// 数据库健康检查结果
#[derive(serde::Serialize)]
pub struct DbHealthInfo {
    pub db_path: String,
    pub sqlite_version: String,
    pub tables: Vec<String>,
    pub healthy: bool,
}

/// 初始化应用数据库连接并执行表结构迁移
pub fn init_database(app: &AppHandle) -> Result<DatabaseState, Box<dyn std::error::Error>> {
    let app_data_dir = app.path().app_data_dir()?;
    std::fs::create_dir_all(&app_data_dir)?;
    let db_path = app_data_dir.join("app.db");

    let conn = Connection::open(&db_path)?;

    // 启用 WAL 模式以提升本地并发与读写性能
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;

    // 执行迁移建表
    schema::run_migrations(&conn)?;

    Ok(DatabaseState {
        conn: Mutex::new(conn),
        db_path,
    })
}

/// 暴露给前端/诊断使用的健康检查命令
#[tauri::command]
pub fn db_health_check(state: State<'_, DatabaseState>) -> Result<DbHealthInfo, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;

    let sqlite_version: String = conn
        .query_row("SELECT sqlite_version()", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
        .map_err(|e| e.to_string())?;

    let tables = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .collect();

    Ok(DbHealthInfo {
        db_path: state.db_path.to_string_lossy().to_string(),
        sqlite_version,
        tables,
        healthy: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_migrations() {
        let conn = Connection::open_in_memory().unwrap();
        schema::run_migrations(&conn).unwrap();

        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
            .unwrap();
        let tables: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();

        assert_eq!(tables, vec!["parse_history", "tasks"]);
    }

    #[test]
    fn test_batch_operations() {
        let mut conn = Connection::open_in_memory().unwrap();
        schema::run_migrations(&conn).unwrap();

        // 1. 测试历史批量插入与上限裁剪
        let mut batch_history = Vec::new();
        for i in 0..60 {
            batch_history.push(history::HistoryItemRecord {
                url: format!("https://example.com/video/{}", i),
                title: format!("Video Title {}", i),
                time: 1000 + i,
            });
        }
        history::add_history_batch(&mut conn, &batch_history).unwrap();

        let list = history::get_recent_history(&conn).unwrap();
        assert_eq!(list.len(), 50);
        assert_eq!(list[0].url, "https://example.com/video/59");

        // 2. 测试任务批量插入
        let batch_tasks = vec![
            tasks::DownloadTaskRecord {
                id: "task_1".to_string(),
                url: "https://example.com/1".to_string(),
                title: "Task 1".to_string(),
                thumbnail: "".to_string(),
                format_label: "".to_string(),
                status: "queued".to_string(),
                percent: 0.0,
                speed: "".to_string(),
                eta: "".to_string(),
                downloaded: "".to_string(),
                total: "".to_string(),
                file_size_bytes: None,
                logs: vec![],
                error: None,
                output_file: None,
                created_at: 1000,
                params: serde_json::json!({}),
            },
            tasks::DownloadTaskRecord {
                id: "task_2".to_string(),
                url: "https://example.com/2".to_string(),
                title: "Task 2".to_string(),
                thumbnail: "".to_string(),
                format_label: "".to_string(),
                status: "completed".to_string(),
                percent: 100.0,
                speed: "".to_string(),
                eta: "".to_string(),
                downloaded: "10MB".to_string(),
                total: "10MB".to_string(),
                file_size_bytes: Some(10485760),
                logs: vec![],
                error: None,
                output_file: Some("/path/to/2.mp4".to_string()),
                created_at: 2000,
                params: serde_json::json!({}),
            },
        ];

        tasks::upsert_tasks_batch(&mut conn, &batch_tasks).unwrap();
        let fetched_tasks = tasks::get_all_tasks(&conn).unwrap();
        assert_eq!(fetched_tasks.len(), 2);
    }
}
