//! Browser extension bridge bound exclusively to the local loopback interface.

use axum::{
    extract::State,
    http::{HeaderMap, HeaderValue, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashSet, VecDeque},
    sync::Mutex,
};
use tauri::{AppHandle, Emitter, Manager};

const BRIDGE_ADDR: &str = "127.0.0.1:17654";
const MAX_COOKIES: usize = 20_000;

#[derive(Clone)]
struct BridgeState {
    app: AppHandle,
}

#[derive(Default)]
pub(crate) struct BrowserBridgeState(Mutex<VecDeque<BrowserImportResult>>);

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CookieImport {
    url: String,
    request_id: String,
    cookies: Vec<BrowserCookie>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BrowserCookie {
    domain: String,
    host_only: bool,
    path: String,
    secure: bool,
    #[allow(dead_code)]
    http_only: bool,
    expiration_date: Option<f64>,
    name: String,
    value: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BrowserImportResult {
    url: String,
    request_id: String,
    cookie_file: Option<String>,
    cookie_count: usize,
}

pub(crate) fn start(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let Ok(listener) = tokio::net::TcpListener::bind(BRIDGE_ADDR).await else {
            eprintln!("browser bridge unavailable: {BRIDGE_ADDR} is already in use");
            return;
        };
        let router = Router::new()
            .route("/v1/health", get(health).options(preflight))
            .route("/v1/import", post(import).options(preflight))
            .with_state(BridgeState { app });
        if let Err(error) = axum::serve(listener, router).await {
            eprintln!("browser bridge stopped: {error}");
        }
    });
}

async fn health(headers: HeaderMap) -> Response {
    response(
        &headers,
        StatusCode::OK,
        Json(serde_json::json!({ "app": "ydl-gui", "version": 1 })),
    )
}

async fn preflight(headers: HeaderMap, method: Method) -> Response {
    if method != Method::OPTIONS || extension_origin(&headers).is_none() {
        return StatusCode::FORBIDDEN.into_response();
    }
    response(&headers, StatusCode::NO_CONTENT, ())
}

async fn import(
    State(state): State<BridgeState>,
    headers: HeaderMap,
    Json(payload): Json<CookieImport>,
) -> Response {
    if extension_origin(&headers).is_none() {
        return StatusCode::FORBIDDEN.into_response();
    }
    match persist_import(&state.app, payload).await {
        Ok(result) => {
            if let Ok(mut pending) = state.app.state::<BrowserBridgeState>().0.lock() {
                pending.push_back(result.clone());
            }
            let _ = state.app.emit("browser-extension-import-ready", ());
            if let Some(window) = state.app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
            response(&headers, StatusCode::OK, Json(result))
        }
        Err(message) => response(
            &headers,
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": message })),
        ),
    }
}

#[tauri::command]
pub(crate) fn take_browser_extension_imports(
    state: tauri::State<'_, BrowserBridgeState>,
) -> Vec<BrowserImportResult> {
    state
        .0
        .lock()
        .map(|mut pending| pending.drain(..).collect())
        .unwrap_or_default()
}

async fn persist_import(
    app: &AppHandle,
    payload: CookieImport,
) -> Result<BrowserImportResult, String> {
    if !payload.url.starts_with("https://") && !payload.url.starts_with("http://") {
        return Err("invalid_url".into());
    }
    if payload.request_id.len() < 8 || payload.request_id.len() > 128 {
        return Err("invalid_request_id".into());
    }
    if payload.cookies.len() > MAX_COOKIES {
        return Err("too_many_cookies".into());
    }

    let mut lines = Vec::with_capacity(payload.cookies.len() + 2);
    lines.push("# Netscape HTTP Cookie File".to_string());
    lines.push("# Generated locally by YDL GUI Browser Extension".to_string());
    let mut seen = HashSet::new();
    for cookie in payload.cookies {
        validate_field(&cookie.domain)?;
        validate_field(&cookie.path)?;
        validate_field(&cookie.name)?;
        validate_field(&cookie.value)?;
        if cookie.domain.is_empty() || cookie.name.is_empty() || !cookie.path.starts_with('/') {
            return Err("invalid_cookie".into());
        }
        let key = format!("{}\0{}\0{}", cookie.domain, cookie.path, cookie.name);
        if !seen.insert(key) {
            continue;
        }
        let domain = &cookie.domain;
        lines.push(format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}",
            domain,
            if cookie.host_only { "FALSE" } else { "TRUE" },
            cookie.path,
            if cookie.secure { "TRUE" } else { "FALSE" },
            cookie
                .expiration_date
                .filter(|v| v.is_finite() && *v > 0.0)
                .map_or(0, |v| v.floor() as i64),
            cookie.name,
            cookie.value
        ));
    }

    let cookie_file = if lines.len() > 2 {
        let path = crate::utils::get_cookie_path(app)?;
        let temporary = path.with_extension("txt.tmp");
        tokio::fs::write(&temporary, format!("{}\n", lines.join("\n")))
            .await
            .map_err(|e| format!("err_save_cookie:{e}"))?;
        if tokio::fs::rename(&temporary, &path).await.is_err() {
            let _ = tokio::fs::remove_file(&path).await;
            tokio::fs::rename(&temporary, &path)
                .await
                .map_err(|e| format!("err_save_cookie:{e}"))?;
        }
        Some(path.to_string_lossy().to_string())
    } else {
        None
    };

    Ok(BrowserImportResult {
        url: payload.url,
        request_id: payload.request_id,
        cookie_file,
        cookie_count: lines.len().saturating_sub(2),
    })
}

fn validate_field(value: &str) -> Result<(), String> {
    if value.contains(['\t', '\r', '\n']) {
        Err("invalid_cookie_field".into())
    } else {
        Ok(())
    }
}

fn extension_origin(headers: &HeaderMap) -> Option<&str> {
    let origin = headers.get("origin")?.to_str().ok()?;
    (origin.starts_with("chrome-extension://") || origin.starts_with("moz-extension://"))
        .then_some(origin)
}

fn response<T: IntoResponse>(headers: &HeaderMap, status: StatusCode, body: T) -> Response {
    let mut response = (status, body).into_response();
    if let Some(origin) = extension_origin(headers).and_then(|v| HeaderValue::from_str(v).ok()) {
        response
            .headers_mut()
            .insert("access-control-allow-origin", origin);
        response
            .headers_mut()
            .insert("vary", HeaderValue::from_static("Origin"));
        response.headers_mut().insert(
            "access-control-allow-methods",
            HeaderValue::from_static("GET, POST, OPTIONS"),
        );
        response.headers_mut().insert(
            "access-control-allow-headers",
            HeaderValue::from_static("Content-Type"),
        );
        response.headers_mut().insert(
            "access-control-allow-private-network",
            HeaderValue::from_static("true"),
        );
    }
    response
}
