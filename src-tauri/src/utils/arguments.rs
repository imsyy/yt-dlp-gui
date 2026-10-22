//! 传递给 yt-dlp 的外部工具与插件参数。

use tauri::AppHandle;

use super::paths::{get_ffmpeg_path, get_js_runtime_path, get_plugin_dir};
use super::source::get_js_runtime;

/// 将当前选择的 FFmpeg 目录显式交给 yt-dlp，解决 GUI 进程 PATH 不完整的问题。
pub fn build_ffmpeg_location_args(app: &AppHandle) -> Vec<String> {
    let Ok(ffmpeg_path) = get_ffmpeg_path(app) else {
        return vec![];
    };
    if !ffmpeg_path.exists() {
        return vec![];
    }
    let location = ffmpeg_path
        .parent()
        .unwrap_or(&ffmpeg_path)
        .to_string_lossy()
        .to_string();
    vec!["--ffmpeg-location".to_string(), location]
}

/// 如果插件目录存在，返回 --plugin-dirs 参数
pub fn build_plugin_args(app: &AppHandle) -> Vec<String> {
    if let Ok(plugin_dir) = get_plugin_dir(app) {
        if plugin_dir.exists() {
            return vec![
                "--plugin-dirs".to_string(),
                plugin_dir.to_string_lossy().to_string(),
            ];
        }
    }
    vec![]
}

/// Explicitly select one runtime so yt-dlp's default Deno cannot override it.
pub fn build_js_runtime_args(app: &AppHandle) -> Vec<String> {
    let runtime = get_js_runtime();
    let path = get_js_runtime_path(app).ok();
    runtime_args(runtime, path.as_deref())
}

fn runtime_args(runtime: &str, path: Option<&std::path::Path>) -> Vec<String> {
    let runtime = path
        .map(|path| format!("{}:{}", runtime, path.to_string_lossy()))
        .unwrap_or_else(|| runtime.to_string());
    vec![
        "--no-js-runtimes".to_string(),
        "--js-runtimes".to_string(),
        runtime,
    ]
}

/// 解析用户自定义命令行参数字符串为独立的参数列表（支持单双引号与转义）
pub fn split_command_line_args(input: &str) -> Result<Vec<String>, String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut started = false;
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '\'' if !in_double_quote => {
                started = true;
                in_single_quote = !in_single_quote;
            }
            '"' if !in_single_quote => {
                started = true;
                in_double_quote = !in_double_quote;
            }
            '\\' if in_double_quote => {
                if let Some(&next) = chars.peek() {
                    if next == '"' || next == '\\' {
                        current.push(chars.next().unwrap());
                        continue;
                    }
                }
                current.push('\\');
            }
            c if c.is_whitespace() && !in_single_quote && !in_double_quote => {
                if started {
                    args.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            c => {
                started = true;
                current.push(c);
            }
        }
    }
    if in_single_quote || in_double_quote {
        return Err("Unclosed quote in custom yt-dlp arguments".to_string());
    }
    if started {
        args.push(current);
    }
    Ok(args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_node_exclusively_and_preserves_windows_paths() {
        assert_eq!(
            runtime_args(
                "node",
                Some(std::path::Path::new(r"C:\Program Files\nodejs\node.exe"))
            ),
            [
                "--no-js-runtimes",
                "--js-runtimes",
                r"node:C:\Program Files\nodejs\node.exe"
            ]
        );
        assert_eq!(
            runtime_args("bun", None),
            ["--no-js-runtimes", "--js-runtimes", "bun"]
        );
    }

    #[test]
    fn test_split_command_line_args() {
        let input = r#"--sleep-requests 2 --sleep-interval 10 --user-agent "Mozilla/5.0 (Windows NT 10.0)""#;
        let args = split_command_line_args(input).unwrap();
        assert_eq!(
            args,
            vec![
                "--sleep-requests",
                "2",
                "--sleep-interval",
                "10",
                "--user-agent",
                "Mozilla/5.0 (Windows NT 10.0)"
            ]
        );
    }

    #[test]
    fn preserves_empty_values_and_windows_paths() {
        assert_eq!(
            split_command_line_args(
                r#"--proxy "" -P "temp:C:\My Downloads" --replace-in-metadata title 'ad' ''"#
            )
            .unwrap(),
            [
                "--proxy",
                "",
                "-P",
                r"temp:C:\My Downloads",
                "--replace-in-metadata",
                "title",
                "ad",
                ""
            ]
        );
    }

    #[test]
    fn rejects_unclosed_quotes_instead_of_silently_changing_arguments() {
        assert!(split_command_line_args("--proxy \"unfinished").is_err());
        assert!(split_command_line_args("--user-agent 'unfinished").is_err());
    }
}
