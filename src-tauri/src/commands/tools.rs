/// 工具箱命令：封面下载、字幕下载、直播弹幕获取
use crate::utils;
use serde_json::Value;
use tauri::AppHandle;

#[cfg(target_os = "windows")]
use super::CREATE_NO_WINDOW;

/// 通用工具命令执行器（--skip-download 模式，不下载视频本身）
async fn run_ytdlp_tool(
    app: &AppHandle,
    url: &str,
    download_dir: &str,
    extra_args: Vec<String>,
    cookie_file: Option<&str>,
    proxy: Option<&str>,
) -> Result<String, String> {
    let ytdlp_path = utils::get_ytdlp_path(app)?;
    if !ytdlp_path.exists() {
        return Err("yt-dlp 未安装，请先在设置中下载".to_string());
    }

    let mut args = vec![
        "--skip-download".to_string(),
        "--ignore-config".to_string(),
        "--color".to_string(),
        "never".to_string(),
        "--windows-filenames".to_string(),
    ];
    args.extend(utils::build_js_runtime_args(app));

    let output_template = std::path::PathBuf::from(download_dir)
        .join("%(title).200s.%(ext)s")
        .to_string_lossy()
        .to_string();
    args.push("-o".to_string());
    args.push(output_template);

    args.extend(extra_args);

    if let Some(cf) = cookie_file {
        if !cf.is_empty() {
            args.push("--cookies".to_string());
            args.push(cf.to_string());
        }
    }
    if let Some(p) = proxy {
        if !p.is_empty() {
            args.push("--proxy".to_string());
            args.push(p.to_string());
        }
    }

    args.push(url.to_string());

    let mut cmd = tokio::process::Command::new(&ytdlp_path);
    cmd.args(&args)
        .env("PYTHONUTF8", "1")
        .env("PYTHONIOENCODING", "utf-8");
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output = cmd
        .output()
        .await
        .map_err(|e| format!("运行 yt-dlp 失败: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if output.status.success() {
        Ok(stdout.to_string())
    } else {
        let error_lines: Vec<&str> = stderr.lines().filter(|l| l.contains("ERROR:")).collect();
        let msg = if error_lines.is_empty() {
            stderr.trim().to_string()
        } else {
            error_lines.join("\n")
        };
        Err(msg)
    }
}

/// 轻量获取视频封面列表（跳过格式检查，速度更快）
#[tauri::command]
pub async fn tool_fetch_thumbnails(
    app: AppHandle,
    url: String,
    cookie_file: Option<String>,
    proxy: Option<String>,
) -> Result<Value, String> {
    let ytdlp_path = utils::get_ytdlp_path(&app)?;
    if !ytdlp_path.exists() {
        return Err("yt-dlp 未安装，请先在设置中下载".to_string());
    }

    let mut args = vec![
        "-J".to_string(),
        "--ignore-config".to_string(),
        "--color".to_string(),
        "never".to_string(),
        "--no-check-formats".to_string(),
        "--no-playlist".to_string(),
    ];
    args.extend(utils::build_js_runtime_args(&app));

    if let Some(ref cf) = cookie_file {
        if !cf.is_empty() {
            args.push("--cookies".to_string());
            args.push(cf.clone());
        }
    }
    if let Some(ref p) = proxy {
        if !p.is_empty() {
            args.push("--proxy".to_string());
            args.push(p.clone());
        }
    }

    args.push(url);

    let mut cmd = tokio::process::Command::new(&ytdlp_path);
    cmd.args(&args)
        .env("PYTHONUTF8", "1")
        .env("PYTHONIOENCODING", "utf-8");
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output = cmd
        .output()
        .await
        .map_err(|e| format!("运行 yt-dlp 失败: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    if let Some(json_str) = stdout
        .lines()
        .find(|line| line.trim_start().starts_with('{'))
    {
        return serde_json::from_str(json_str).map_err(|e| format!("解析视频信息失败: {}", e));
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let error_lines: Vec<&str> = stderr.lines().filter(|l| l.contains("ERROR:")).collect();
    let msg = if error_lines.is_empty() {
        stderr.trim().to_string()
    } else {
        error_lines.join("\n")
    };
    Err(msg)
}

/// 将指定 URL 的图片下载到指定文件路径（另存为）
#[tauri::command]
pub async fn tool_save_thumbnail(
    url: String,
    file_path: String,
    proxy: Option<String>,
) -> Result<(), String> {
    let mut builder = reqwest::Client::builder();
    if let Some(ref p) = proxy {
        if !p.is_empty() {
            let reqwest_proxy =
                reqwest::Proxy::all(p).map_err(|e| format!("代理配置错误: {}", e))?;
            builder = builder.proxy(reqwest_proxy);
        }
    }
    let client = builder
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("下载封面失败: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("下载封面失败: HTTP {}", response.status()));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("读取封面数据失败: {}", e))?;

    // 确保父目录存在
    if let Some(parent) = std::path::Path::new(&file_path).parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("创建目录失败: {}", e))?;
    }

    tokio::fs::write(&file_path, &bytes)
        .await
        .map_err(|e| format!("保存封面文件失败: {}", e))?;

    Ok(())
}

/// 下载视频封面图
#[tauri::command]
pub async fn tool_download_thumbnail(
    app: AppHandle,
    url: String,
    download_dir: String,
    cookie_file: Option<String>,
    proxy: Option<String>,
) -> Result<String, String> {
    run_ytdlp_tool(
        &app,
        &url,
        &download_dir,
        vec![
            "--write-thumbnail".to_string(),
            "--convert-thumbnails".to_string(),
            "jpg".to_string(),
        ],
        cookie_file.as_deref(),
        proxy.as_deref(),
    )
    .await
}

/// 获取视频可用字幕列表（返回 subtitles + automatic_captions）
#[tauri::command]
pub async fn tool_fetch_subtitles(
    app: AppHandle,
    url: String,
    cookie_file: Option<String>,
    proxy: Option<String>,
) -> Result<Value, String> {
    let ytdlp_path = utils::get_ytdlp_path(&app)?;
    if !ytdlp_path.exists() {
        return Err("yt-dlp 未安装，请先在设置中下载".to_string());
    }

    let mut args = vec![
        "-J".to_string(),
        "--ignore-config".to_string(),
        "--color".to_string(),
        "never".to_string(),
        "--no-check-formats".to_string(),
        "--no-playlist".to_string(),
    ];
    args.extend(utils::build_js_runtime_args(&app));

    if let Some(ref cf) = cookie_file {
        if !cf.is_empty() {
            args.push("--cookies".to_string());
            args.push(cf.clone());
        }
    }
    if let Some(ref p) = proxy {
        if !p.is_empty() {
            args.push("--proxy".to_string());
            args.push(p.clone());
        }
    }

    args.push(url);

    let mut cmd = tokio::process::Command::new(&ytdlp_path);
    cmd.args(&args)
        .env("PYTHONUTF8", "1")
        .env("PYTHONIOENCODING", "utf-8");
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output = cmd
        .output()
        .await
        .map_err(|e| format!("运行 yt-dlp 失败: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    if let Some(json_str) = stdout
        .lines()
        .find(|line| line.trim_start().starts_with('{'))
    {
        let info: Value =
            serde_json::from_str(json_str).map_err(|e| format!("解析视频信息失败: {}", e))?;

        // 只返回字幕相关字段和标题
        let result = serde_json::json!({
            "title": info.get("title").cloned().unwrap_or(Value::Null),
            "subtitles": info.get("subtitles").cloned().unwrap_or(Value::Object(Default::default())),
            "automatic_captions": info.get("automatic_captions").cloned().unwrap_or(Value::Object(Default::default())),
        });
        return Ok(result);
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let error_lines: Vec<&str> = stderr.lines().filter(|l| l.contains("ERROR:")).collect();
    let msg = if error_lines.is_empty() {
        stderr.trim().to_string()
    } else {
        error_lines.join("\n")
    };
    Err(msg)
}

/// 下载单个字幕文件并另存为
#[tauri::command]
pub async fn tool_save_subtitle(
    url: String,
    file_path: String,
    proxy: Option<String>,
) -> Result<(), String> {
    let mut builder = reqwest::Client::builder();
    if let Some(ref p) = proxy {
        if !p.is_empty() {
            let reqwest_proxy =
                reqwest::Proxy::all(p).map_err(|e| format!("代理配置错误: {}", e))?;
            builder = builder.proxy(reqwest_proxy);
        }
    }
    let client = builder
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("下载字幕失败: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("下载字幕失败: HTTP {}", response.status()));
    }

    let text = response
        .text()
        .await
        .map_err(|e| format!("读取字幕数据失败: {}", e))?;

    if let Some(parent) = std::path::Path::new(&file_path).parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("创建目录失败: {}", e))?;
    }

    tokio::fs::write(&file_path, &text)
        .await
        .map_err(|e| format!("保存字幕文件失败: {}", e))?;

    Ok(())
}

/// 下载 URL 文本内容并返回（用于前端获取字幕文本做合并处理）
#[tauri::command]
pub async fn tool_download_text(url: String, proxy: Option<String>) -> Result<String, String> {
    let mut builder = reqwest::Client::builder();
    if let Some(ref p) = proxy {
        if !p.is_empty() {
            let reqwest_proxy =
                reqwest::Proxy::all(p).map_err(|e| format!("代理配置错误: {}", e))?;
            builder = builder.proxy(reqwest_proxy);
        }
    }
    let client = builder
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("下载失败: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("下载失败: HTTP {}", response.status()));
    }

    response
        .text()
        .await
        .map_err(|e| format!("读取文本失败: {}", e))
}

/// 将文本内容保存到指定文件路径
#[tauri::command]
pub async fn tool_save_text_to_file(content: String, file_path: String) -> Result<(), String> {
    if let Some(parent) = std::path::Path::new(&file_path).parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("创建目录失败: {}", e))?;
    }

    tokio::fs::write(&file_path, &content)
        .await
        .map_err(|e| format!("保存文件失败: {}", e))?;

    Ok(())
}

/// 下载视频字幕文件（旧接口，保留兼容）
#[tauri::command]
pub async fn tool_download_subtitles(
    app: AppHandle,
    url: String,
    download_dir: String,
    sub_langs: String,
    write_auto_subs: bool,
    cookie_file: Option<String>,
    proxy: Option<String>,
) -> Result<String, String> {
    let mut extra = vec![
        "--write-subs".to_string(),
        "--sub-langs".to_string(),
        sub_langs,
    ];
    if write_auto_subs {
        extra.push("--write-auto-subs".to_string());
    }
    run_ytdlp_tool(
        &app,
        &url,
        &download_dir,
        extra,
        cookie_file.as_deref(),
        proxy.as_deref(),
    )
    .await
}

/// 下载直播弹幕/聊天记录
#[tauri::command]
pub async fn tool_download_live_chat(
    app: AppHandle,
    url: String,
    download_dir: String,
    cookie_file: Option<String>,
    proxy: Option<String>,
) -> Result<String, String> {
    run_ytdlp_tool(
        &app,
        &url,
        &download_dir,
        vec![
            "--write-subs".to_string(),
            "--sub-langs".to_string(),
            "live_chat".to_string(),
        ],
        cookie_file.as_deref(),
        proxy.as_deref(),
    )
    .await
}
