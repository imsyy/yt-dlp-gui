use rusqlite::{Connection, Result};

const CURRENT_SCHEMA_VERSION: i32 = 1;

/// 获取当前数据库 user_version
pub fn get_schema_version(conn: &Connection) -> Result<i32> {
    conn.query_row("PRAGMA user_version", [], |row| row.get(0))
}

/// 执行数据表与索引初始化与版本迁移
pub fn run_migrations(conn: &Connection) -> Result<()> {
    let current_version = get_schema_version(conn)?;

    if current_version < CURRENT_SCHEMA_VERSION {
        conn.execute_batch(
            r#"
            -- 移除不再由数据库托管的应用设置表（保留前端 localStorage 管理）
            DROP TABLE IF EXISTS app_settings;

            -- 核心下载任务表
            CREATE TABLE IF NOT EXISTS tasks (
                id TEXT PRIMARY KEY,
                url TEXT NOT NULL,
                title TEXT NOT NULL,
                thumbnail TEXT NOT NULL DEFAULT '',
                format_label TEXT NOT NULL DEFAULT '',
                status TEXT NOT NULL,
                percent REAL NOT NULL DEFAULT 0,
                speed TEXT NOT NULL DEFAULT '',
                eta TEXT NOT NULL DEFAULT '',
                downloaded TEXT NOT NULL DEFAULT '',
                total TEXT NOT NULL DEFAULT '',
                file_size_bytes INTEGER,
                logs TEXT NOT NULL DEFAULT '[]',
                error TEXT,
                output_file TEXT,
                created_at INTEGER NOT NULL,
                params_json TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
            CREATE INDEX IF NOT EXISTS idx_tasks_created_at ON tasks(created_at DESC);

            -- URL 解析历史表（保留最近记录）
            CREATE TABLE IF NOT EXISTS parse_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                url TEXT UNIQUE NOT NULL,
                title TEXT NOT NULL,
                accessed_at INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_parse_history_accessed_at ON parse_history(accessed_at DESC);
            "#,
        )?;

        ensure_task_columns(conn)?;
        conn.execute(&format!("PRAGMA user_version = {}", CURRENT_SCHEMA_VERSION), [])?;
    } else {
        ensure_task_columns(conn)?;
    }

    Ok(())
}

/// 兼容补齐 tasks 表缺失字段并严格传播错误
fn ensure_task_columns(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare("PRAGMA table_info(tasks)")?;
    let existing_columns: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<Vec<_>, _>>()?;

    if !existing_columns.iter().any(|c| c == "thumbnail") {
        conn.execute(
            "ALTER TABLE tasks ADD COLUMN thumbnail TEXT NOT NULL DEFAULT ''",
            [],
        )?;
    }
    if !existing_columns.iter().any(|c| c == "format_label") {
        conn.execute(
            "ALTER TABLE tasks ADD COLUMN format_label TEXT NOT NULL DEFAULT ''",
            [],
        )?;
    }
    if !existing_columns.iter().any(|c| c == "file_size_bytes") {
        conn.execute(
            "ALTER TABLE tasks ADD COLUMN file_size_bytes INTEGER",
            [],
        )?;
    }
    if !existing_columns.iter().any(|c| c == "logs") {
        conn.execute(
            "ALTER TABLE tasks ADD COLUMN logs TEXT NOT NULL DEFAULT '[]'",
            [],
        )?;
    }

    Ok(())
}
