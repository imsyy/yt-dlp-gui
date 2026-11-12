//! Facebook 页面链接采集。进度与日志使用短标题消息；
//! 结果通过 TXT Blob 下载保存，下载完成后由 Rust 回读并通知主窗口。
//! 采集结果保存在当前页面内，停止不会清空；刷新或关闭页面会结束该会话。

use serde::Serialize;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

/// 采集窗口的固定 label：全局唯一，重复打开时聚焦已存在的窗口而非新建。
const COLLECTOR_LABEL: &str = "fb-collector";

/// 目标站点：当前仅 Facebook。保留参数化入口，后续可扩展其他站点。
const FACEBOOK_HOME: &str = "https://www.facebook.com/";

/// 结果 txt 的文件名标记：用于在 on_download 里识别“这是我们的采集结果”，
/// 而不是用户自己在页面上点的其它下载。
const RESULT_FILENAME_TAG: &str = "ydl-collector-links";

/// document.title 信道前缀（仅进度 / 日志用）。
const TITLE_PREFIX: &str = "__YDLC__";

/// 转发给主窗口的采集事件名（主窗口是本地页面，用标准事件系统即可）。
const EVENT_PROGRESS: &str = "fb-collector-progress";
const EVENT_RESULT: &str = "fb-collector-result";
const EVENT_LOG: &str = "fb-collector-log";

/// 采集会话状态：仅保存最近一次结果文件路径（预留给前端再次读取）。
#[derive(Default)]
pub(crate) struct CollectorState {
    last_file: Mutex<Option<PathBuf>>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ResultPayload {
    urls: Vec<String>,
    /// 结果同时落盘的 txt 路径，前端可展示“已保存到…”。
    file: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ProgressPayload {
    scanning: bool,
    count: usize,
    scans: usize,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct LogPayload {
    message: String,
}

/// 注入到目标页面的采集脚本。作为 initialization_script 在每次顶层导航、
/// 页面自身脚本之前运行，因此必须自行按 hostname 判断是否注入 UI，并且
/// 用 window.__ydl 全局保存状态，避免 SPA 多次执行产生多个 reels 实例。
///
/// 进度 / 日志走 document.title；结果走 Blob 下载（文件名含 RESULT_FILENAME_TAG）。
/// 参数占位 `__PANEL_TEXT__` 由 Rust 在注入前替换为本地化后的面板文案 JSON。
const COLLECTOR_SCRIPT: &str = r#"
(function () {
  try {
    if (!/(^|\.)facebook\.com$/.test(location.hostname)) return;
  } catch (e) { return; }

  // 全局单例：所有状态挂在 window.__ydl 上，SPA 多次注入也只初始化一次。
  var G = window.__ydl;
  if (!G) {
    G = window.__ydl = {
      reels: new Set(),
      running: false,
      stopRequested: false,
      seq: 0,
      queue: [],
      sending: false,
      savedTitle: document.title,
    };
  }
  var T = __PANEL_TEXT__;
  G.T = T;

  // ---- 进度 / 日志：document.title 出站 ----
  function pump() {
    if (G.sending) return;
    if (G.queue.length === 0) { try { document.title = G.savedTitle; } catch (e) {} return; }
    G.sending = true;
    var msg = G.queue.shift();
    try { document.title = msg + "|#" + (G.seq++); } catch (e) {}
    setTimeout(function () { G.sending = false; pump(); }, 50);
  }
  function post(s) { G.queue.push(s); pump(); }
  function enc(s) { try { return encodeURIComponent(String(s)); } catch (e) { return ""; } }
  function log(m) { post("__YDLC__|L|" + enc(m)); }
  function progress(scanning, scans) {
    post("__YDLC__|P|" + (scanning ? 1 : 0) + "|" + G.reels.size + "|" + (scans || 0));
    var badge = document.getElementById("ydl-collector-count");
    if (badge) badge.textContent = String(G.reels.size);
  }

  // ---- 采集 ----
  function collect() {
    var anchors = document.querySelectorAll("a[href]");
    for (var i = 0; i < anchors.length; i++) {
      try {
        var u = new URL(anchors[i].href, location.origin);
        var m1 = u.pathname.match(/^\/reel\/(\d+)/);
        if (m1) { G.reels.add("https://www.facebook.com/reel/" + m1[1] + "/"); continue; }
        var m2 = u.pathname.match(/\/videos\/(\d+)/);
        if (m2) { G.reels.add("https://www.facebook.com/videos/" + m2[1] + "/"); continue; }
        var m3 = u.pathname.match(/^\/watch\/?$/);
        if (m3 && u.searchParams.get("v")) {
          G.reels.add("https://www.facebook.com/watch/?v=" + u.searchParams.get("v"));
        }
      } catch (e) {}
    }
  }
  function sleep(ms) { return new Promise(function (r) { setTimeout(r, ms); }); }

  G.run = async function (opts) {
    if (G.running) return;
    G.running = true;
    G.stopRequested = false;
    var maxScans = opts.maxScans || 500;
    var interval = opts.interval || 1800;
    var stopAfterIdle = opts.stopAfterIdle || 10;
    var unchanged = 0, lastCount = G.reels.size;
    setBtn(true);
    log(T.started);
    for (var i = 1; i <= maxScans; i++) {
      if (G.stopRequested) { log(T.stoppedByUser); break; }
      collect();
      progress(true, i);
      window.scrollTo(0, document.body.scrollHeight);
      await sleep(interval);
      collect();
      if (G.reels.size === lastCount) { unchanged++; } else { unchanged = 0; lastCount = G.reels.size; }
      if (unchanged >= stopAfterIdle) { log(T.noMore); break; }
    }
    collect();
    G.running = false;
    setBtn(false);
    progress(false, 0);
    log(T.done.replace("{count}", String(G.reels.size)));
  };

  G.stop = function () { G.stopRequested = true; };

  // 发送：把链接写成 txt 触发下载（文件名含 tag，供 Rust 识别并落盘 + 导入）。
  // 不依赖任何 IPC；采集中、停止后都能点。
  G.send = function () {
    collect();
    // Set is iterable, not array-like: slice.call(Set) silently returns [].
    var list = Array.from(G.reels);
    if (list.length === 0) { log(T.emptyResult); return; }
    var text = list.join("\n");
    try {
      var blob = new Blob([text], { type: "text/plain;charset=utf-8" });
      var a = document.createElement("a");
      a.href = URL.createObjectURL(blob);
      a.download = "ydl-collector-links-" + list.length + ".txt";
      document.body.appendChild(a);
      a.click();
      setTimeout(function () { try { URL.revokeObjectURL(a.href); a.remove(); } catch (e) {} }, 1000);
      log(T.sent.replace("{count}", String(list.length)));
    } catch (e) {
      log("send failed: " + e);
    }
  };

  function setBtn(isRunning) {
    var s = document.getElementById("ydl-collector-start");
    var t = document.getElementById("ydl-collector-stop");
    if (s) { s.disabled = isRunning; s.style.opacity = isRunning ? "0.5" : "1"; }
    if (t) { t.disabled = !isRunning; t.style.opacity = isRunning ? "1" : "0.5"; }
  }

  function mountPanel() {
    if (document.getElementById("ydl-collector-panel")) return;
    if (!document.body) { setTimeout(mountPanel, 300); return; }
    var panel = document.createElement("div");
    panel.id = "ydl-collector-panel";
    panel.style.cssText = [
      "position:fixed", "right:16px", "bottom:16px", "z-index:2147483647",
      "background:#1f2937", "color:#f9fafb", "border-radius:12px",
      "box-shadow:0 8px 24px rgba(0,0,0,0.35)", "padding:12px 14px",
      "font-family:system-ui,-apple-system,sans-serif", "font-size:13px",
      "width:230px", "user-select:none"
    ].join(";");
    panel.innerHTML =
      '<div style="font-weight:600;margin-bottom:8px;display:flex;align-items:center;justify-content:space-between">' +
        '<span>YDL ' + T.title + '</span>' +
        '<span style="background:#10b981;border-radius:10px;padding:1px 8px;font-size:12px">' +
          '<span id="ydl-collector-count">0</span></span>' +
      '</div>' +
      '<div style="font-size:12px;color:#9ca3af;line-height:1.5;margin-bottom:8px">' + T.hint + '</div>' +
      '<div style="display:flex;gap:6px">' +
        '<button id="ydl-collector-start" style="flex:1;cursor:pointer;border:none;border-radius:8px;padding:6px 0;background:#10b981;color:#fff;font-size:12px">' + T.start + '</button>' +
        '<button id="ydl-collector-stop" style="flex:1;cursor:pointer;border:none;border-radius:8px;padding:6px 0;background:#6b7280;color:#fff;font-size:12px" disabled>' + T.stop + '</button>' +
        '<button id="ydl-collector-send" style="flex:1;cursor:pointer;border:none;border-radius:8px;padding:6px 0;background:#2563eb;color:#fff;font-size:12px">' + T.send + '</button>' +
      '</div>';
    document.body.appendChild(panel);
    document.getElementById("ydl-collector-start").addEventListener("click", function () {
      G.run({ maxScans: 500, interval: 1800, stopAfterIdle: 10 });
    });
    document.getElementById("ydl-collector-stop").addEventListener("click", function () { G.stop(); });
    document.getElementById("ydl-collector-send").addEventListener("click", function () { G.send(); });
    setBtn(G.running);
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", mountPanel);
  } else {
    mountPanel();
  }
})();
"#;

/// 打开（或聚焦）站内链接采集窗口。
///
/// `panel_text` 由前端传入面板文案 JSON 字符串（已本地化），注入脚本用它渲染
/// 悬浮控制面板，使采集窗口语言跟随应用设置。
#[tauri::command]
pub async fn open_collector_window(app: AppHandle, panel_text: String) -> Result<(), String> {
    // 已存在则聚焦复用，避免重复登录与多窗口混乱。
    if let Some(existing) = app.get_webview_window(COLLECTOR_LABEL) {
        let _ = existing.unminimize();
        let _ = existing.show();
        existing.set_focus().map_err(|e| e.to_string())?;
        return Ok(());
    }

    // 面板文案是可信的本地 JSON，但仍做最小转义，避免注入脚本被破坏。
    let safe_text = sanitize_panel_text(&panel_text);
    let script = COLLECTOR_SCRIPT.replace("__PANEL_TEXT__", &safe_text);

    let app_for_title = app.clone();
    let app_for_dl = app.clone();
    WebviewWindowBuilder::new(
        &app,
        COLLECTOR_LABEL,
        WebviewUrl::External(
            FACEBOOK_HOME
                .parse()
                .map_err(|e| format!("err_parse_url:{e}"))?,
        ),
    )
    .title("YDL Collector")
    .inner_size(1120.0, 820.0)
    .initialization_script(&script)
    // 进度 / 日志：document.title 信道。
    .on_document_title_changed(move |_window, title| {
        handle_title_message(&app_for_title, &title);
    })
    // 结果：拦截采集脚本触发的 txt 下载，改写落盘路径，完成后读取并转发。
    .on_download(move |_webview, event| handle_download(&app_for_dl, event))
    .build()
    .map_err(|e| format!("err_open_collector:{e}"))?;

    Ok(())
}

/// 结果 txt 的落盘目录：应用缓存目录 / ydl-collector，不存在则创建。
fn result_dir(app: &AppHandle) -> Option<PathBuf> {
    let dir = app.path().app_cache_dir().ok()?.join("ydl-collector");
    let _ = std::fs::create_dir_all(&dir);
    Some(dir)
}

/// 处理采集窗口的下载事件：仅接管我们自己触发的结果文件（文件名含 tag）。
/// 返回 true 允许下载继续；对无关下载不干预（同样返回 true）。
fn handle_download(app: &AppHandle, event: tauri::webview::DownloadEvent<'_>) -> bool {
    use tauri::webview::DownloadEvent;
    match event {
        DownloadEvent::Requested { url, destination } => {
            let named_ours = destination
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.contains(RESULT_FILENAME_TAG))
                .unwrap_or(false);
            // Do not intercept unrelated downloads from the remote page.
            let is_ours = named_ours && url.as_str().starts_with("blob:https://www.facebook.com/");
            if is_ours {
                if let Some(dir) = result_dir(app) {
                    let raw = destination
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(str::to_string)
                        .unwrap_or_default();
                    let name = if raw.contains(RESULT_FILENAME_TAG) {
                        raw
                    } else {
                        // 文件名拿不到时兜底带上 tag，便于 Finished 阶段识别
                        format!("{RESULT_FILENAME_TAG}.txt")
                    };
                    *destination = dir.join(name);
                }
            }
            true
        }
        DownloadEvent::Finished {
            url: _,
            path,
            success,
        } => {
            if success {
                if let Some(path) = path {
                    let is_ours = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(|n| n.contains(RESULT_FILENAME_TAG))
                        .unwrap_or(false);
                    if is_ours {
                        forward_result_file(app, &path);
                    }
                }
            }
            true
        }
        _ => true,
    }
}

/// 读取结果 txt、解析出链接、去重，并把结果转发给主窗口（同时附带文件路径）。
fn forward_result_file(app: &AppHandle, path: &std::path::Path) {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return,
    };
    let mut seen = std::collections::HashSet::new();
    let mut urls = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if (line.starts_with("http://") || line.starts_with("https://"))
            && seen.insert(line.to_string())
        {
            urls.push(line.to_string());
        }
    }
    if urls.is_empty() {
        return;
    }
    if let Ok(mut last) = app.state::<CollectorState>().last_file.lock() {
        *last = Some(path.to_path_buf());
    }
    let _ = app.emit(
        EVENT_RESULT,
        ResultPayload {
            urls,
            file: Some(path.to_string_lossy().to_string()),
        },
    );
}

/// 解析注入脚本通过 document.title 发来的进度 / 日志消息，转发给主窗口。
/// 非本协议前缀的标题（用户正常浏览产生的标题）直接忽略。
fn handle_title_message(app: &AppHandle, title: &str) {
    let Some(body) = title.strip_prefix(TITLE_PREFIX) else {
        return;
    };
    let body = match body.rfind("|#") {
        Some(idx) => &body[..idx],
        None => body,
    };
    let mut parts = body.split('|');
    let _ = parts.next(); // body 形如 "|<type>|..."，首段为空
    let Some(kind) = parts.next() else {
        return;
    };
    match kind {
        "P" => {
            let scanning = parts.next().map(|s| s == "1").unwrap_or(false);
            let count = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
            let scans = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
            let _ = app.emit(
                EVENT_PROGRESS,
                ProgressPayload {
                    scanning,
                    count,
                    scans,
                },
            );
        }
        "L" => {
            let message = parts.next().map(percent_decode).unwrap_or_default();
            let _ = app.emit(EVENT_LOG, LogPayload { message });
        }
        _ => {}
    }
}

/// 轻量 percent-decode：仅还原 encodeURIComponent 产生的 %XX 与 UTF-8 字节。
fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = hex_val(bytes[i + 1]);
            let lo = hex_val(bytes[i + 2]);
            if let (Some(h), Some(l)) = (hi, lo) {
                out.push((h << 4) | l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// 仅保留能被 JS 字面量安全嵌入的内容：面板文案理应是合法 JSON 字符串。
fn sanitize_panel_text(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.starts_with('{') && trimmed.ends_with('}') && !trimmed.contains("</") {
        trimmed.to_string()
    } else {
        default_panel_text()
    }
}

/// 面板文案兜底（英文），确保即便前端未传或传入非法 JSON 也不至于白板。
fn default_panel_text() -> String {
    serde_json::json!({
        "title": "Collector",
        "hint": "Scroll & collect video links on this page.",
        "start": "Start",
        "stop": "Stop",
        "send": "Send",
        "started": "Started scrolling & collecting...",
        "stoppedByUser": "Stopped.",
        "noMore": "No new videos for a while, stopped.",
        "done": "Collected {count} links.",
        "sent": "Saved {count} links.",
        "emptyResult": "No links collected yet.",
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_keeps_valid_json_object() {
        let input = r#"{"title":"采集","start":"开始"}"#;
        assert_eq!(sanitize_panel_text(input), input);
    }

    #[test]
    fn sanitize_falls_back_on_non_object() {
        assert_eq!(sanitize_panel_text("not json"), default_panel_text());
        assert_eq!(sanitize_panel_text(""), default_panel_text());
    }

    #[test]
    fn sanitize_rejects_tag_injection() {
        let malicious = r#"{"x":"</script>"}"#;
        assert_eq!(sanitize_panel_text(malicious), default_panel_text());
    }

    #[test]
    fn script_has_download_send_and_globals() {
        let script = COLLECTOR_SCRIPT.replace("__PANEL_TEXT__", &default_panel_text());
        assert!(!script.contains("__PANEL_TEXT__"));
        // 结果走下载：文件名含 tag、用 a.download 触发
        assert!(script.contains("ydl-collector-links-"));
        assert!(script.contains("a.download"));
        // 状态挂 window.__ydl 全局
        assert!(script.contains("window.__ydl"));
    }

    #[test]
    fn percent_decode_restores_utf8_and_comma() {
        assert_eq!(percent_decode("a%2Cb"), "a,b");
        assert_eq!(percent_decode("https%3A%2F%2Fx.com%2F"), "https://x.com/");
        assert_eq!(percent_decode("%E4%B8%AD"), "中");
        assert_eq!(percent_decode("abc%"), "abc%");
    }
}
