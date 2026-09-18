//! 本地 SQLite 嵌入式数据库核心管理模块。

pub mod history;
pub mod schema;
pub mod tasks;
pub mod tool_tasks;

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
    // 锁等待超时，避免极端并发下直接报 SQLITE_BUSY
    conn.pragma_update(None, "busy_timeout", "5000")?;

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

    // 真实完整性校验：integrity_check 首行返回 ok 才算健康
    let healthy: bool = conn
        .query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))
        .map(|first_line| first_line.eq_ignore_ascii_case("ok"))
        .unwrap_or(false);

    Ok(DbHealthInfo {
        db_path: state.db_path.to_string_lossy().to_string(),
        sqlite_version,
        tables,
        healthy,
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

        assert_eq!(
            tables,
            vec!["parse_history", "tasks", "tool_results", "tool_task_states"]
        );
    }

    #[test]
    fn test_tool_task_state_and_latest_result_are_separate() {
        let conn = Connection::open_in_memory().unwrap();
        schema::run_migrations(&conn).unwrap();
        let state = DatabaseState {
            conn: Mutex::new(conn),
            db_path: PathBuf::from(":memory:"),
        };

        let running = tool_tasks::start_state(
            &state,
            "chapters",
            "chapters_run_1",
            "https://example.com/video",
        )
        .unwrap();
        assert_eq!(running.status, "running");
        assert!(tool_tasks::get_result(&state, "chapters").unwrap().is_none());

        tool_tasks::save_json_result(
            &state,
            "chapters",
            "chapters_run_1",
            r#"{"chapters":[]}"#,
            0,
        )
        .unwrap();
        let completed = tool_tasks::finish_state(
            &state,
            "chapters",
            "chapters_run_1",
            "completed",
            None,
        )
        .unwrap()
        .unwrap();
        assert_eq!(completed.status, "completed");
        assert_eq!(
            tool_tasks::get_result(&state, "chapters")
                .unwrap()
                .unwrap()
                .0
                .run_id,
            "chapters_run_1"
        );
    }

    #[test]
    fn test_cancelled_state_is_not_overwritten_by_late_finish() {
        let conn = Connection::open_in_memory().unwrap();
        schema::run_migrations(&conn).unwrap();
        let state = DatabaseState {
            conn: Mutex::new(conn),
            db_path: PathBuf::from(":memory:"),
        };

        tool_tasks::start_state(
            &state,
            "livechat",
            "livechat_run_1",
            "https://example.com/live",
        )
        .unwrap();

        // 用户取消：状态先落为 cancelled
        let cancelled =
            tool_tasks::finish_state(&state, "livechat", "livechat_run_1", "cancelled", None)
                .unwrap()
                .unwrap();
        assert_eq!(cancelled.status, "cancelled");

        // 被杀的进程随后让收尾流程带着错误再调一次 finish_state：必须是无操作，不得覆盖 cancelled
        assert!(tool_tasks::finish_state(
            &state,
            "livechat",
            "livechat_run_1",
            "failed",
            Some("err_exit_code:1"),
        )
        .unwrap()
        .is_none());
        let current = tool_tasks::get_state(&state, "livechat").unwrap().unwrap();
        assert_eq!(current.status, "cancelled");
        assert!(current.error.is_none());
    }

    #[test]
    fn test_late_finish_of_previous_run_does_not_touch_new_run() {
        let conn = Connection::open_in_memory().unwrap();
        schema::run_migrations(&conn).unwrap();
        let state = DatabaseState {
            conn: Mutex::new(conn),
            db_path: PathBuf::from(":memory:"),
        };

        tool_tasks::start_state(&state, "chapters", "chapters_run_1", "https://example.com/a")
            .unwrap();
        assert!(tool_tasks::finish_state(&state, "chapters", "chapters_run_1", "cancelled", None)
            .unwrap()
            .is_some());

        // 取消后必须能立刻开始下一个任务
        let next =
            tool_tasks::start_state(&state, "chapters", "chapters_run_2", "https://example.com/b")
                .unwrap();
        assert_eq!(next.run_id, "chapters_run_2");

        // 旧 run 的迟到收尾不能影响新任务
        assert!(tool_tasks::finish_state(
            &state,
            "chapters",
            "chapters_run_1",
            "failed",
            Some("err_run_ytdlp"),
        )
        .unwrap()
        .is_none());
        let current = tool_tasks::get_state(&state, "chapters").unwrap().unwrap();
        assert_eq!(current.run_id, "chapters_run_2");
        assert_eq!(current.status, "running");
    }

    #[test]
    fn test_start_state_rejects_second_run_while_running() {
        let conn = Connection::open_in_memory().unwrap();
        schema::run_migrations(&conn).unwrap();
        let state = DatabaseState {
            conn: Mutex::new(conn),
            db_path: PathBuf::from(":memory:"),
        };

        tool_tasks::start_state(&state, "comments", "comments_run_1", "https://example.com/a")
            .unwrap();
        let err =
            tool_tasks::start_state(&state, "comments", "comments_run_2", "https://example.com/b")
                .unwrap_err();
        assert_eq!(err, "err_tool_already_running");

        // 不同工具互不阻塞
        tool_tasks::start_state(&state, "chapters", "chapters_run_1", "https://example.com/c")
            .unwrap();
    }

    #[test]
    fn test_update_running_stage_is_noop_after_cancel() {
        let conn = Connection::open_in_memory().unwrap();
        schema::run_migrations(&conn).unwrap();
        let state = DatabaseState {
            conn: Mutex::new(conn),
            db_path: PathBuf::from(":memory:"),
        };

        tool_tasks::start_state(&state, "chapters", "chapters_run_1", "https://example.com/a")
            .unwrap();
        assert!(
            tool_tasks::update_running_stage(&state, "chapters", "chapters_run_1", "fetching")
                .unwrap()
                .is_some()
        );

        tool_tasks::finish_state(&state, "chapters", "chapters_run_1", "cancelled", None).unwrap();

        // 取消后不得再改 stage，否则会把已取消的状态当成仍在运行又推给前端一次
        assert!(
            tool_tasks::update_running_stage(&state, "chapters", "chapters_run_1", "fetching")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn test_running_tool_ids_only_lists_running() {
        let conn = Connection::open_in_memory().unwrap();
        schema::run_migrations(&conn).unwrap();
        let state = DatabaseState {
            conn: Mutex::new(conn),
            db_path: PathBuf::from(":memory:"),
        };

        assert!(tool_tasks::get_running_tool_ids(&state).unwrap().is_empty());

        tool_tasks::start_state(&state, "chapters", "chapters_run_1", "https://example.com/a")
            .unwrap();
        tool_tasks::start_state(&state, "livechat", "livechat_run_1", "https://example.com/b")
            .unwrap();
        assert_eq!(
            tool_tasks::get_running_tool_ids(&state).unwrap(),
            vec!["chapters", "livechat"]
        );

        // 完成与取消都不算运行中，否则底栏会一直显示有任务在跑
        tool_tasks::finish_state(&state, "chapters", "chapters_run_1", "completed", None).unwrap();
        assert_eq!(
            tool_tasks::get_running_tool_ids(&state).unwrap(),
            vec!["livechat"]
        );
        tool_tasks::finish_state(&state, "livechat", "livechat_run_1", "cancelled", None).unwrap();
        assert!(tool_tasks::get_running_tool_ids(&state).unwrap().is_empty());
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
