//! yt-dlp 下载参数构建与校验。

use crate::{commands::support::append_cookie_proxy_args, utils};
use tauri::AppHandle;

use super::model::DownloadParams;

pub(super) fn requires_ffmpeg_merge(params: &DownloadParams) -> bool {
    params.download_mode == "default"
        && params
            .video_format
            .as_deref()
            .is_some_and(|format| !format.is_empty())
        && params
            .audio_format
            .as_deref()
            .is_some_and(|format| !format.is_empty())
        && !params.no_merge
}

/// 根据下载模式与格式选择，构建 `-f` 格式参数。
/// `no_merge` 为 true 时不使用 `+` 拼接，避免触发 ffmpeg 合并
/// （yt-dlp 无 --no-merge-output 选项，用单独格式替代）。
fn build_format_args(params: &DownloadParams) -> Vec<String> {
    match params.download_mode.as_str() {
        "video" => {
            if let Some(ref vf) = params.video_format {
                if !vf.is_empty() {
                    return vec!["-f".to_string(), vf.clone()];
                }
            }
            Vec::new()
        }
        "audio" => {
            if let Some(ref af) = params.audio_format {
                if !af.is_empty() {
                    return vec!["-f".to_string(), af.clone()];
                }
            }
            Vec::new()
        }
        _ => {
            let vf = params.video_format.as_deref().filter(|s| !s.is_empty());
            let af = params.audio_format.as_deref().filter(|s| !s.is_empty());
            match (vf, af) {
                (Some(v), Some(a)) => {
                    if params.no_merge {
                        vec!["-f".to_string(), v.to_string()]
                    } else {
                        vec!["-f".to_string(), format!("{}+{}", v, a)]
                    }
                }
                (Some(v), None) => {
                    if params.no_merge {
                        vec!["-f".to_string(), v.to_string()]
                    } else {
                        vec!["-f".to_string(), format!("{}+bestaudio/{}", v, v)]
                    }
                }
                (None, Some(a)) => {
                    if params.no_merge {
                        vec!["-f".to_string(), a.to_string()]
                    } else {
                        vec!["-f".to_string(), format!("bestvideo+{0}/{0}", a)]
                    }
                }
                _ => Vec::new(),
            }
        }
    }
}

/// 构建 yt-dlp 下载参数
pub(super) fn build_download_args(
    app: &AppHandle,
    params: &DownloadParams,
) -> Result<Vec<String>, String> {
    let mut args: Vec<String> = vec![
        "--newline".to_string(),
        "--ignore-config".to_string(),  // 忽略用户系统配置，防止干扰 GUI
        "--color".to_string(), "never".to_string(),  // 禁用 ANSI 颜色转义序列
        // 使用 --progress-template 输出结构化进度（官方推荐方式，避免解析 stdout 常规输出）
        "--progress-template".to_string(),
        r#"download:PROGRESS_JSON:{"percent":"%(progress._percent_str|0%)s","speed":"%(progress._speed_str|)s","eta":"%(progress._eta_str|)s","downloaded":"%(progress._downloaded_bytes_str|)s","total":"%(progress._total_bytes_str|)s"}"#.to_string(),
    ];

    // JS 运行时（Deno）
    args.extend(utils::build_js_runtime_args(app));
    args.extend(utils::build_ffmpeg_location_args(app));
    args.extend(utils::build_plugin_args(app));
    // YouTube PO Token / visitor_data（如设置）
    args.extend(utils::build_youtube_extractor_args());

    // 格式选择
    args.extend(build_format_args(params));

    // 代理
    if let Some(ref proxy) = params.proxy {
        if !proxy.is_empty() {
            args.push("--proxy".to_string());
            args.push(proxy.clone());
        }
    }

    // 输出路径模板
    let template = params
        .output_template
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or("%(title).200s.%(ext)s");
    let output_template = std::path::PathBuf::from(&params.download_dir)
        .join(template)
        .to_string_lossy()
        .to_string();
    args.push("-o".to_string());
    args.push(output_template);
    args.push("--windows-filenames".to_string());

    // 不覆盖已有文件
    if params.no_overwrites {
        args.push("--no-overwrites".to_string());
    }

    // 并发分片下载
    if let Some(n) = params.concurrent_fragments {
        if n > 1 {
            args.push("--concurrent-fragments".to_string());
            args.push(n.to_string());
        }
    }

    // Cookie 和浏览器 Cookie
    append_cookie_proxy_args(
        &mut args,
        params.cookie_file.as_deref(),
        params.cookie_browser.as_deref(),
        None, // 代理在上方已单独处理
    );

    // 额外选项
    if params.embed_subs {
        args.push("--embed-subs".to_string());
    }
    if params.embed_thumbnail {
        args.push("--embed-thumbnail".to_string());
    }
    if params.embed_metadata {
        args.push("--embed-metadata".to_string());
    }
    // 嵌入章节标记
    if params.embed_chapters {
        args.push("--embed-chapters".to_string());
    }
    // SponsorBlock：移除赞助片段
    if params.sponsorblock_remove {
        args.push("--sponsorblock-remove".to_string());
        args.push("all".to_string());
    }
    // 提取音频模式
    if params.extract_audio {
        args.push("-x".to_string());
        if let Some(ref fmt) = params.audio_convert_format {
            if !fmt.is_empty() {
                args.push("--audio-format".to_string());
                args.push(fmt.clone());
            }
        }
    }
    if let Some(ref fmt) = params.recode_format {
        if !fmt.is_empty() {
            args.push("--recode-video".to_string());
            args.push(fmt.clone());
        }
    }
    if let Some(ref rate) = params.limit_rate {
        if !rate.is_empty() {
            args.push("-r".to_string());
            args.push(rate.clone());
        }
    }
    // 自定义 FFmpeg 后处理参数
    if let Some(ref ffmpeg_args) = params.ffmpeg_args {
        if !ffmpeg_args.is_empty() {
            args.push("--postprocessor-args".to_string());
            args.push(format!("FFmpeg:{}", ffmpeg_args));
        }
    }

    // 字幕
    append_subtitle_args(&mut args, &params.subtitles);

    // 时间范围裁剪（仅在有实际裁剪范围时添加，避免 *0-inf 触发不必要的 ffmpeg 处理）
    // 前端已将 time picker 值转换为秒数
    let has_start = params.start_time.is_some_and(|t| t > 0.0);
    let has_end = params.end_time.is_some();
    if has_start || has_end {
        let start = params.start_time.unwrap_or(0.0);
        let end_str = params
            .end_time
            .map(|t| format!("{}", t))
            .unwrap_or_else(|| "inf".to_string());
        args.push("--download-sections".to_string());
        args.push(format!("*{}-{}", start, end_str));
    }

    // 播放列表
    if params.no_playlist {
        args.push("--no-playlist".to_string());
    } else if let Some(ref items) = params.playlist_items {
        if !items.is_empty() {
            args.push("--playlist-items".to_string());
            args.push(items.clone());
        }
    }

    // URL（必须放在最后）
    args.push(params.url.clone());

    Ok(args)
}

/// 将前端的 `sub:<lang>` / `auto:<lang>` 选择转换为 yt-dlp 参数。
/// `--sub-langs` 对普通和自动字幕共用，因此语言只需去重后传递一次。
fn append_subtitle_args(args: &mut Vec<String>, selections: &[String]) {
    let mut write_manual = false;
    let mut write_auto = false;
    let mut languages: Vec<&str> = Vec::new();

    for selection in selections {
        let language = match selection.split_once(':') {
            Some(("sub", language)) => {
                write_manual = true;
                language
            }
            Some(("auto", language)) => {
                write_auto = true;
                language
            }
            // 兼容旧版本持久化的无前缀字幕值。
            _ => {
                write_manual = true;
                selection.as_str()
            }
        };
        if !language.is_empty() && !languages.contains(&language) {
            languages.push(language);
        }
    }

    if languages.is_empty() {
        return;
    }
    if write_manual {
        args.push("--write-subs".to_string());
    }
    if write_auto {
        args.push("--write-auto-subs".to_string());
    }
    args.push("--sub-langs".to_string());
    args.push(languages.join(","));
}

#[cfg(test)]
mod subtitle_arg_tests {
    use super::append_subtitle_args;

    #[test]
    fn manual_subtitles_strip_source_prefix() {
        let mut args = Vec::new();
        append_subtitle_args(&mut args, &["sub:en".to_string()]);
        assert_eq!(args, ["--write-subs", "--sub-langs", "en"]);
    }

    #[test]
    fn automatic_subtitles_enable_auto_caption_download() {
        let mut args = Vec::new();
        append_subtitle_args(&mut args, &["auto:zh-Hans".to_string()]);
        assert_eq!(args, ["--write-auto-subs", "--sub-langs", "zh-Hans"]);
    }

    #[test]
    fn mixed_subtitles_enable_both_sources_and_deduplicate_languages() {
        let mut args = Vec::new();
        append_subtitle_args(
            &mut args,
            &[
                "sub:en".to_string(),
                "auto:ja".to_string(),
                "auto:en".to_string(),
            ],
        );
        assert_eq!(
            args,
            ["--write-subs", "--write-auto-subs", "--sub-langs", "en,ja"]
        );
    }
}

#[cfg(test)]
mod ffmpeg_requirement_tests {
    use super::{build_format_args, requires_ffmpeg_merge, DownloadParams};

    fn params() -> DownloadParams {
        DownloadParams {
            id: "test".to_string(),
            url: "https://example.com/video".to_string(),
            download_dir: ".".to_string(),
            download_mode: "default".to_string(),
            video_format: Some("137".to_string()),
            audio_format: Some("140".to_string()),
            cookie_file: None,
            cookie_browser: None,
            proxy: None,
            output_template: None,
            concurrent_fragments: None,
            no_overwrites: false,
            embed_subs: false,
            embed_thumbnail: false,
            embed_metadata: false,
            embed_chapters: false,
            sponsorblock_remove: false,
            extract_audio: false,
            audio_convert_format: None,
            no_merge: false,
            recode_format: None,
            limit_rate: None,
            ffmpeg_args: None,
            subtitles: Vec::new(),
            start_time: None,
            end_time: None,
            no_playlist: false,
            playlist_items: None,
        }
    }

    #[test]
    fn separate_video_and_audio_require_ffmpeg_for_default_download() {
        assert!(requires_ffmpeg_merge(&params()));
    }

    #[test]
    fn explicit_no_merge_allows_separate_output_files() {
        let mut value = params();
        value.no_merge = true;
        assert!(!requires_ffmpeg_merge(&value));
    }

    #[test]
    fn no_merge_uses_single_format_without_plus_concatenation() {
        let mut value = params();
        value.no_merge = true;
        let args = build_format_args(&value);
        assert_eq!(args, ["-f", "137"]);
    }

    #[test]
    fn merge_mode_uses_plus_concatenation() {
        let args = build_format_args(&params());
        assert_eq!(args, ["-f", "137+140"]);
    }
}
