//! 工具箱统一后台任务入口：状态与结果读取分离。

use super::{
    fetch_live_chat, tool_fetch_chapters, tool_fetch_comments, tool_fetch_subtitles,
    tool_fetch_thumbnails,
};
use crate::db::{self, tool_tasks};
use crate::platform::process::{self, ProcessRegistry};
use serde_json::Value;
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::{AppHandle, Emitter, Manager, State};

static RUN_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub async fn cleanup_tool_cache(app: AppHandle) {
    let Ok(root) = app.path().app_data_dir() else {
        return;
    };
    let directory = root.join("tool-results");
    if directory.exists() {
        let _ = tokio::fs::remove_dir_all(&directory).await;
    }
}

fn make_run_id(tool_id: &str) -> String {
    format!(
        "{}_{}_{}",
        tool_id,
        tool_tasks::now_millis(),
        RUN_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    )
}

fn optional_string(params: &Value, key: &str) -> Option<String> {
    params.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn result_total(tool_id: &str, value: &Value) -> i64 {
    if tool_id == "subtitles" {
        return ["subtitles", "automatic_captions"]
            .iter()
            .filter_map(|key| value.get(key).and_then(Value::as_object))
            .map(|tracks| tracks.len() as i64)
            .sum();
    }
    if tool_id == "livechat" {
        return value.as_array().map(|items| items.len() as i64).unwrap_or_default();
    }
    let key = match tool_id {
        "thumbnail" => "thumbnails",
        "chapters" => "chapters",
        "comments" => "comments",
        _ => return 0,
    };
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|items| items.len() as i64)
        .unwrap_or_default()
}

async fn run_tool(
    app: &AppHandle,
    tool_id: &str,
    run_id: &str,
    url: &str,
    params: &Value,
) -> Result<(), String> {
    let cookie_file = optional_string(params, "cookieFile");
    let cookie_browser = optional_string(params, "cookieBrowser");
    let proxy = optional_string(params, "proxy");

    let value = match tool_id {
        "livechat" => {
            let messages = fetch_live_chat(
                app,
                run_id,
                url,
                cookie_file.as_deref(),
                cookie_browser.as_deref(),
                proxy.as_deref(),
            )
            .await?;
            serde_json::to_value(messages).map_err(|error| error.to_string())?
        }
        "thumbnail" => {
            tool_fetch_thumbnails(
                app.clone(),
                run_id.to_owned(),
                url.to_owned(),
                cookie_file,
                cookie_browser,
                proxy,
            )
            .await?
        }
        "subtitles" => {
            tool_fetch_subtitles(
                app.clone(),
                run_id.to_owned(),
                url.to_owned(),
                cookie_file,
                cookie_browser,
                proxy,
            )
            .await?
        }
        "chapters" => {
            tool_fetch_chapters(
                app.clone(),
                run_id.to_owned(),
                url.to_owned(),
                cookie_file,
                cookie_browser,
                proxy,
            )
            .await?
        }
        "comments" => {
            tool_fetch_comments(
                app.clone(),
                run_id.to_owned(),
                url.to_owned(),
                params
                    .get("maxComments")
                    .and_then(Value::as_u64)
                    .unwrap_or(100) as u32,
                optional_string(params, "sort").unwrap_or_else(|| "top".into()),
                cookie_file,
                cookie_browser,
                proxy,
            )
            .await?
        }
        _ => return Err("err_unknown_tool".into()),
    };
    let total = result_total(tool_id, &value);
    let result_json = serde_json::to_string(&value).map_err(|error| error.to_string())?;
    let db = app.state::<db::DatabaseState>();
    tool_tasks::save_json_result(&db, tool_id, run_id, &result_json, total)
}

async fn execute_task(app: AppHandle, tool_id: String, run_id: String, url: String, params: Value) {
    let db = app.state::<db::DatabaseState>();
    if let Ok(Some(state)) = tool_tasks::update_running_stage(&db, &tool_id, &run_id, "fetching") {
        let _ = app.emit("tool-task-state-changed", state);
    }

    let result = run_tool(&app, &tool_id, &run_id, &url, &params).await;

    let db = app.state::<db::DatabaseState>();
    let state = match result {
        Ok(()) => tool_tasks::finish_state(&db, &tool_id, &run_id, "completed", None),
        Err(error) => tool_tasks::finish_state(&db, &tool_id, &run_id, "failed", Some(&error)),
    };
    if let Ok(Some(state)) = state {
        let _ = app.emit("tool-task-state-changed", state);
    }
}

#[tauri::command]
pub async fn tool_start_task(
    app: AppHandle,
    state: State<'_, db::DatabaseState>,
    tool_id: String,
    url: String,
    params: Value,
) -> Result<tool_tasks::ToolTaskStateRecord, String> {
    let run_id = make_run_id(&tool_id);
    let task_state = tool_tasks::start_state(&state, &tool_id, &run_id, &url)?;
    let _ = app.emit("tool-task-state-changed", task_state.clone());
    tauri::async_runtime::spawn(execute_task(app, tool_id, run_id, url, params));
    Ok(task_state)
}

#[tauri::command]
pub fn tool_get_task_state(
    state: State<'_, db::DatabaseState>,
    tool_id: String,
) -> Result<Option<tool_tasks::ToolTaskStateRecord>, String> {
    tool_tasks::get_state(&state, &tool_id)
}

/// 正在运行的工具 id 列表，供状态栏全局指示使用。
#[tauri::command]
pub fn tool_get_running_tasks(
    state: State<'_, db::DatabaseState>,
) -> Result<Vec<String>, String> {
    tool_tasks::get_running_tool_ids(&state)
}

/// 取消的状态迁移：**先落 cancelled，再执行 kill**。
///
/// 顺序不能颠倒：`kill_process` 要等 taskkill 执行完才返回，若先杀再落状态，
/// 被杀的进程会让收尾流程抢先写入 failed，而 `finish_state` 的 running 守卫随后
/// 会把 cancelled 挡掉，状态就再也回不到 cancelled（用户会看到一条伪错误）。
/// 把顺序收进这个函数，调用方无法写反。
///
/// 返回 None 表示状态已不是 running（例如刚好在点击前完成），此时不执行 kill，
/// 避免误杀一个已经成功的进程。
fn cancel_state_then_kill(
    db: &db::DatabaseState,
    tool_id: &str,
    run_id: &str,
    kill: impl FnOnce(),
) -> Result<Option<tool_tasks::ToolTaskStateRecord>, String> {
    let Some(updated) = tool_tasks::finish_state(db, tool_id, run_id, "cancelled", None)? else {
        return Ok(None);
    };
    kill();
    Ok(Some(updated))
}

/// 终止指定工具正在执行的任务：先把状态落为 cancelled，再杀掉子进程树。
#[tauri::command]
pub async fn tool_cancel_task(
    app: AppHandle,
    state: State<'_, db::DatabaseState>,
    registry: State<'_, ProcessRegistry>,
    tool_id: String,
) -> Result<(), String> {
    let Some(current) = tool_tasks::get_state(&state, &tool_id)? else {
        return Err("err_tool_state_missing".into());
    };
    let Some(updated) = cancel_state_then_kill(&state, &tool_id, &current.run_id, || {
        if let Some(pid) = registry.take(&current.run_id) {
            let _ = process::kill_process(pid);
        }
    })?
    else {
        return Ok(());
    };
    let _ = app.emit("tool-task-state-changed", updated);
    Ok(())
}

#[tauri::command]
pub fn tool_get_result(
    state: State<'_, db::DatabaseState>,
    tool_id: String,
) -> Result<Option<tool_tasks::ToolResultRecord>, String> {
    Ok(tool_tasks::get_result(&state, &tool_id)?.map(|result| result.0))
}



#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::schema;
    use rusqlite::Connection;
    use std::path::PathBuf;
    use std::sync::Mutex;

    fn memory_db() -> db::DatabaseState {
        let conn = Connection::open_in_memory().unwrap();
        schema::run_migrations(&conn).unwrap();
        db::DatabaseState {
            conn: Mutex::new(conn),
            db_path: PathBuf::from(":memory:"),
        }
    }

    #[test]
    fn test_cancel_writes_state_before_killing() {
        let db = memory_db();
        tool_tasks::start_state(&db, "chapters", "chapters_run_1", "https://example.com/a")
            .unwrap();

        let mut killed = false;
        let updated = cancel_state_then_kill(&db, "chapters", "chapters_run_1", || {
            killed = true;
            // 杀进程的那一刻状态必须已经是 cancelled；若这里读到 running，
            // 说明顺序被写反了，被杀的进程随后会让收尾流程把状态覆盖成 failed。
            let current = tool_tasks::get_state(&db, "chapters").unwrap().unwrap();
            assert_eq!(current.status, "cancelled");
        })
        .unwrap();

        assert!(killed);
        assert_eq!(updated.unwrap().status, "cancelled");
    }

    #[test]
    fn test_cancel_does_not_kill_task_that_already_finished() {
        let db = memory_db();
        tool_tasks::start_state(&db, "chapters", "chapters_run_1", "https://example.com/a")
            .unwrap();
        tool_tasks::finish_state(&db, "chapters", "chapters_run_1", "completed", None).unwrap();

        let mut killed = false;
        let updated =
            cancel_state_then_kill(&db, "chapters", "chapters_run_1", || killed = true).unwrap();

        // 刚好在点击前完成：状态已不是 running，不得误杀一个已经成功的进程
        assert!(!killed);
        assert!(updated.is_none());
    }
}
