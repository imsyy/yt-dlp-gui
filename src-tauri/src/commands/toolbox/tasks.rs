//! 工具箱统一后台任务入口：状态与结果读取分离。

use super::{
    fetch_live_chat_to_jsonl, tool_fetch_chapters, tool_fetch_comments, tool_fetch_subtitles,
    tool_fetch_thumbnails, LiveChatMessage,
};
use crate::db::{self, tool_tasks};
use crate::platform::process::{self, ProcessRegistry};
use serde_json::Value;
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::io::{AsyncBufReadExt, AsyncSeekExt, AsyncWriteExt, BufReader};

static RUN_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn matches_chat(
    message: &LiveChatMessage,
    query: &str,
    regex: Option<&regex::Regex>,
) -> bool {
    if query.is_empty() {
        return true;
    }
    if let Some(regex) = regex {
        return regex.is_match(&message.message) || regex.is_match(&message.author);
    }
    let query = query.to_lowercase();
    message.message.to_lowercase().contains(&query)
        || message.author.to_lowercase().contains(&query)
}

pub async fn cleanup_tool_cache(app: AppHandle) {
    let Ok(root) = app.path().app_data_dir() else {
        return;
    };
    let directory = root.join("tool-results/livechat");
    let db = app.state::<db::DatabaseState>();
    let referenced = tool_tasks::get_result(&db, "livechat")
        .ok()
        .flatten()
        .and_then(|result| result.1)
        .map(|path| root.join(path));
    let Ok(mut entries) = tokio::fs::read_dir(&directory).await else {
        return;
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) == Some("part")
            || referenced.as_ref().map_or(true, |current| current != &path)
        {
            let _ = tokio::fs::remove_file(path).await;
        }
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

    if tool_id == "livechat" {
        let (relative, total) = fetch_live_chat_to_jsonl(
            app,
            url,
            cookie_file.as_deref(),
            cookie_browser.as_deref(),
            proxy.as_deref(),
            run_id,
        )
        .await?;
        let db = app.state::<db::DatabaseState>();
        let previous = tool_tasks::save_file_result(&db, tool_id, run_id, &relative, total)?;
        if let Some(previous) = previous.filter(|path| path != &relative) {
            if let Ok(root) = app.path().app_data_dir() {
                let _ = tokio::fs::remove_file(root.join(previous)).await;
            }
        }
        return Ok(());
    }

    let value = match tool_id {
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

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveChatPage {
    items: Vec<LiveChatMessage>,
    next_cursor: Option<u64>,
    has_more: bool,
    total: i64,
}

#[tauri::command]
pub async fn tool_read_live_chat_page(
    app: AppHandle,
    state: State<'_, db::DatabaseState>,
    run_id: String,
    cursor: Option<u64>,
    limit: Option<u32>,
    query: Option<String>,
    use_regex: Option<bool>,
) -> Result<LiveChatPage, String> {
    let Some((meta, relative_file)) = tool_tasks::get_result(&state, "livechat")? else {
        return Err("err_tool_result_missing".into());
    };
    if meta.run_id != run_id {
        return Err("err_tool_result_stale".into());
    }
    let relative_file = relative_file.ok_or("err_tool_result_file_missing")?;
    let root = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    let path = root.join(relative_file);
    let mut file = tokio::fs::File::open(path)
        .await
        .map_err(|error| error.to_string())?;
    file.seek(std::io::SeekFrom::Start(cursor.unwrap_or_default()))
        .await
        .map_err(|error| error.to_string())?;
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    let mut items = Vec::new();
    let limit = limit.unwrap_or(200).clamp(1, 500) as usize;
    let query = query.unwrap_or_default();
    let regex = use_regex
        .unwrap_or(false)
        .then(|| regex::RegexBuilder::new(&query).case_insensitive(true).build())
        .transpose()
        .map_err(|error| error.to_string())?;
    let mut position = cursor.unwrap_or_default();
    while items.len() < limit {
        line.clear();
        let read = reader
            .read_line(&mut line)
            .await
            .map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        position += read as u64;
        let message: LiveChatMessage = match serde_json::from_str(&line) {
            Ok(message) => message,
            Err(_) => continue,
        };
        if matches_chat(&message, &query, regex.as_ref()) {
            items.push(message);
        }
    }
    let has_more = !reader
        .fill_buf()
        .await
        .map_err(|error| error.to_string())?
        .is_empty();
    Ok(LiveChatPage {
        items,
        next_cursor: has_more.then_some(position),
        has_more,
        total: meta.total,
    })
}

#[tauri::command]
pub async fn tool_export_live_chat(
    app: AppHandle,
    state: State<'_, db::DatabaseState>,
    run_id: String,
    file_path: String,
    format: String,
    selected_fields: Vec<String>,
    query: Option<String>,
    use_regex: Option<bool>,
) -> Result<(), String> {
    let Some((meta, relative_file)) = tool_tasks::get_result(&state, "livechat")? else {
        return Err("err_tool_result_missing".into());
    };
    if meta.run_id != run_id {
        return Err("err_tool_result_stale".into());
    }
    let source = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?
        .join(relative_file.ok_or("err_tool_result_file_missing")?);
    let input = tokio::fs::File::open(source)
        .await
        .map_err(|error| error.to_string())?;
    let mut lines = BufReader::new(input).lines();
    let mut output = tokio::fs::File::create(file_path)
        .await
        .map_err(|error| error.to_string())?;
    let query = query.unwrap_or_default();
    let regex = use_regex
        .unwrap_or(false)
        .then(|| regex::RegexBuilder::new(&query).case_insensitive(true).build())
        .transpose()
        .map_err(|error| error.to_string())?;
    let is_json = format.eq_ignore_ascii_case("json");
    if is_json {
        output
            .write_all(b"[\n")
            .await
            .map_err(|error| error.to_string())?;
    } else {
        let header = selected_fields
            .iter()
            .map(|field| format!("\"{}\"", field.replace('"', "\"\"")))
            .collect::<Vec<_>>()
            .join(",");
        output
            .write_all(format!("{header}\r\n").as_bytes())
            .await
            .map_err(|error| error.to_string())?;
    }
    let mut first = true;
    while let Some(line) = lines.next_line().await.map_err(|error| error.to_string())? {
        let message: LiveChatMessage = match serde_json::from_str(&line) {
            Ok(message) => message,
            Err(_) => continue,
        };
        if !matches_chat(&message, &query, regex.as_ref()) {
            continue;
        }
        let value = serde_json::to_value(&message).map_err(|error| error.to_string())?;
        if is_json {
            let filtered = selected_fields
                .iter()
                .filter_map(|field| value.get(field).map(|value| (field.clone(), value.clone())))
                .collect::<serde_json::Map<_, _>>();
            if !first {
                output
                    .write_all(b",\n")
                    .await
                    .map_err(|error| error.to_string())?;
            }
            output
                .write_all(
                    serde_json::to_string(&filtered)
                        .map_err(|error| error.to_string())?
                        .as_bytes(),
                )
                .await
                .map_err(|error| error.to_string())?;
        } else {
            let row = selected_fields
                .iter()
                .map(|field| {
                    let text = value
                        .get(field)
                        .map(|value| {
                            value
                                .as_str()
                                .map(str::to_owned)
                                .unwrap_or_else(|| value.to_string())
                        })
                        .unwrap_or_default();
                    format!("\"{}\"", text.replace('"', "\"\""))
                })
                .collect::<Vec<_>>()
                .join(",");
            output
                .write_all(format!("{row}\r\n").as_bytes())
                .await
                .map_err(|error| error.to_string())?;
        }
        first = false;
    }
    if is_json {
        output
            .write_all(b"\n]\n")
            .await
            .map_err(|error| error.to_string())?;
    }
    output.flush().await.map_err(|error| error.to_string())
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
