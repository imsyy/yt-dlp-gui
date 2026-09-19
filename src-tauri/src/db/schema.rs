use rusqlite::{Connection, Result};

const CURRENT_SCHEMA_VERSION: i32 = 6;

pub fn get_schema_version(conn: &Connection) -> Result<i32> {
    conn.query_row("PRAGMA user_version", [], |row| row.get(0))
}

pub fn run_migrations(conn: &Connection) -> Result<()> {
    let current_version = get_schema_version(conn)?;

    conn.execute_batch(
        r#"
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

    if current_version < 4 {
        conn.execute_batch(
            r#"
            DROP TABLE IF EXISTS tool_snapshots;
            DROP TABLE IF EXISTS tool_results;

            CREATE TABLE tool_task_states (
                tool_id TEXT PRIMARY KEY NOT NULL,
                run_id TEXT NOT NULL,
                status TEXT NOT NULL,
                url TEXT NOT NULL,
                stage TEXT,
                progress REAL,
                error TEXT,
                started_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );

            CREATE TABLE tool_results (
                tool_id TEXT PRIMARY KEY NOT NULL,
                run_id TEXT NOT NULL,
                result_type TEXT NOT NULL,
                result_json TEXT,
                result_file TEXT,
                total INTEGER NOT NULL DEFAULT 0,
                completed_at INTEGER NOT NULL
            );
            CREATE INDEX idx_tool_results_run_id ON tool_results(run_id);
            "#,
        )?;
    }

    if current_version < 5 {
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS channels (
                id TEXT PRIMARY KEY NOT NULL,
                url TEXT UNIQUE NOT NULL,
                title TEXT NOT NULL,
                uploader TEXT NOT NULL DEFAULT '',
                uploader_id TEXT NOT NULL DEFAULT '',
                avatar TEXT NOT NULL DEFAULT '',
                banner TEXT NOT NULL DEFAULT '',
                description TEXT NOT NULL DEFAULT '',
                platform TEXT NOT NULL DEFAULT 'youtube',
                subscriber_count INTEGER,
                video_count INTEGER NOT NULL DEFAULT 0,
                last_synced_at INTEGER,
                sync_status TEXT NOT NULL DEFAULT 'idle',
                sync_error TEXT,
                created_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_channels_updated ON channels(last_synced_at DESC);

            CREATE TABLE IF NOT EXISTS channel_videos (
                id TEXT PRIMARY KEY NOT NULL,
                channel_id TEXT NOT NULL,
                video_id TEXT NOT NULL,
                url TEXT NOT NULL,
                title TEXT NOT NULL,
                thumbnail TEXT NOT NULL DEFAULT '',
                duration REAL,
                view_count INTEGER,
                published_at INTEGER,
                published_accuracy TEXT NOT NULL DEFAULT 'approx',
                content_type TEXT NOT NULL DEFAULT 'video',
                created_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_channel_videos_channel ON channel_videos(channel_id);
            CREATE INDEX IF NOT EXISTS idx_channel_videos_pub ON channel_videos(channel_id, published_at DESC);
            CREATE INDEX IF NOT EXISTS idx_channel_videos_type ON channel_videos(channel_id, content_type);
            "#,
        )?;
    }

    if current_version < 6 {
        ensure_channel_video_accuracy_column(conn)?;
    }

    conn.execute(
        "UPDATE tool_task_states SET status = 'interrupted', error = 'application restarted', updated_at = ?1 WHERE status = 'running'",
        [now_millis()],
    )?;
    conn.pragma_update(None, "user_version", CURRENT_SCHEMA_VERSION)?;
    Ok(())
}

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or_default()
}

fn ensure_channel_video_accuracy_column(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare("PRAGMA table_info(channel_videos)")?;
    let existing_columns: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<Vec<_>, _>>()?;

    if !existing_columns
        .iter()
        .any(|column| column == "published_accuracy")
    {
        // 存量行的日期全部来自 flat-playlist 近似值，默认 'approx' 语义正确；
        // 精确值由日期校准任务回填为 'exact'。
        conn.execute(
            "ALTER TABLE channel_videos ADD COLUMN published_accuracy TEXT NOT NULL DEFAULT 'approx'",
            [],
        )?;
    }
    Ok(())
}

fn ensure_task_columns(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare("PRAGMA table_info(tasks)")?;
    let existing_columns: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<Vec<_>, _>>()?;

    if !existing_columns.iter().any(|column| column == "thumbnail") {
        conn.execute(
            "ALTER TABLE tasks ADD COLUMN thumbnail TEXT NOT NULL DEFAULT ''",
            [],
        )?;
    }
    if !existing_columns
        .iter()
        .any(|column| column == "format_label")
    {
        conn.execute(
            "ALTER TABLE tasks ADD COLUMN format_label TEXT NOT NULL DEFAULT ''",
            [],
        )?;
    }
    if !existing_columns
        .iter()
        .any(|column| column == "file_size_bytes")
    {
        conn.execute("ALTER TABLE tasks ADD COLUMN file_size_bytes INTEGER", [])?;
    }
    if !existing_columns.iter().any(|column| column == "logs") {
        conn.execute(
            "ALTER TABLE tasks ADD COLUMN logs TEXT NOT NULL DEFAULT '[]'",
            [],
        )?;
    }
    Ok(())
}
