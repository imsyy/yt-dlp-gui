//! yt-dlp 下载参数构建与校验。

use crate::{commands::support::append_cookie_proxy_args, utils};
use tauri::AppHandle;

use super::model::DownloadParams;

pub(super) fn requires_ffmpeg(params: &DownloadParams) -> bool {
    (params.download_mode == "default"
        && !params.no_merge
        && (params.playlist_items.is_some()
            || !params.video_has_audio
            || params
                .audio_format
                .as_deref()
                .is_some_and(|format| !format.is_empty()))
        && (params
            .video_format
            .as_deref()
            .is_some_and(|format| !format.is_empty())
            || params
                .audio_format
                .as_deref()
                .is_some_and(|format| format.contains('+'))))
        || requires_audio_extraction(params)
        || params.embed_subs
        || params.embed_thumbnail
        || params.embed_metadata
        || params.embed_chapters
        || params.sponsorblock_remove
        || params
            .recode_format
            .as_deref()
            .is_some_and(|s| !s.is_empty())
        || params
            .remux_format
            .as_deref()
            .is_some_and(|s| !s.is_empty() && s != "auto")
        || params.start_time.is_some_and(|t| t > 0.0)
        || params.end_time.is_some()
}

fn requires_audio_extraction(params: &DownloadParams) -> bool {
    params.extract_audio
        || (params.download_mode == "audio"
            && params
                .audio_format
                .as_deref()
                .is_none_or(|format| format.is_empty()))
}

/// 根据下载模式与格式选择，构建 `-f` 格式参数。
/// `no_merge` 为 true 时用逗号分别下载各格式，不丢弃已选音轨。
fn build_format_args(params: &DownloadParams) -> Vec<String> {
    let mut args = build_format_selector_args(params);
    let selected_video = params
        .video_format
        .as_deref()
        .is_some_and(|format| !format.is_empty());
    let selected_audio = params
        .audio_format
        .as_deref()
        .is_some_and(|format| !format.is_empty());
    let has_explicit_format = match params.download_mode.as_str() {
        _ if params.extract_audio => selected_audio,
        "audio" => selected_audio,
        "video" => selected_video,
        _ => selected_video || selected_audio,
    };
    // Format IDs belong to individual videos. A different entry in the same post
    // or playlist may not provide the first entry's chosen format IDs.
    if !params.no_playlist
        && params.playlist_items.is_some()
        && has_explicit_format
        && args.len() == 2
    {
        let fallback = match params.download_mode.as_str() {
            _ if params.extract_audio => "bestaudio/best",
            "audio" => "bestaudio",
            "video" => "bestvideo/best",
            _ if params.video_format.as_deref().is_none_or(str::is_empty) => "bestaudio/best",
            _ if params.no_merge => "bestvideo/best",
            _ => "bestvideo*+bestaudio/best",
        };
        if params.download_mode == "default" && params.no_merge && args[1].contains(',') {
            args[1] = args[1]
                .split(',')
                .enumerate()
                .map(|(index, format)| {
                    let fallback = if index == 0
                        && params
                            .video_format
                            .as_deref()
                            .is_some_and(|format| !format.is_empty())
                    {
                        "bestvideo/best"
                    } else {
                        "bestaudio/best"
                    };
                    format!("{format}/{fallback}")
                })
                .collect::<Vec<_>>()
                .join(",");
        } else {
            args[1] = format!("{}/{}", args[1], fallback);
        }
    }
    if params.download_mode == "default"
        && !params.no_merge
        && !params.extract_audio
        && params.audio_format.as_deref().is_some_and(|value| {
            !value.is_empty() && (value.contains('+') || params.video_has_audio)
        })
    {
        args.push("--audio-multistreams".to_string());
    }
    args
}

fn build_format_selector_args(params: &DownloadParams) -> Vec<String> {
    // Audio extraction should fetch the chosen audio directly when available.
    // Downloading and merging video first wastes bandwidth and can make FFmpeg
    // extract the video's embedded soundtrack instead of the selected language.
    if params.extract_audio {
        let audio = params
            .audio_format
            .as_deref()
            .and_then(|value| value.split('+').find(|format| !format.is_empty()))
            .unwrap_or("bestaudio/best");
        return vec!["-f".to_string(), audio.to_string()];
    }
    match params.download_mode.as_str() {
        "video" => {
            if let Some(ref vf) = params.video_format {
                if !vf.is_empty() {
                    return vec!["-f".to_string(), vf.clone()];
                }
            }
            vec!["-f".to_string(), "bestvideo/best".to_string()]
        }
        "audio" => {
            if let Some(ref af) = params.audio_format {
                if !af.is_empty() {
                    return vec![
                        "-f".to_string(),
                        af.split('+').next().unwrap_or(af).to_string(),
                    ];
                }
            }
            vec!["-f".to_string(), "bestaudio/best".to_string()]
        }
        _ => {
            let vf = params.video_format.as_deref().filter(|s| !s.is_empty());
            let af = params.audio_format.as_deref().filter(|s| !s.is_empty());
            match (vf, af) {
                (Some(v), Some(a)) => {
                    if params.no_merge {
                        vec!["-f".to_string(), format!("{},{}", v, a.replace('+', ","))]
                    } else {
                        vec!["-f".to_string(), format!("{}+{}", v, a)]
                    }
                }
                (Some(v), None) => {
                    if params.no_merge || params.video_has_audio {
                        vec!["-f".to_string(), v.to_string()]
                    } else {
                        vec![
                            "-f".to_string(),
                            format!(
                                "{0}[acodec!=none]/{0}+bestaudio/best[vcodec!=none][acodec!=none]",
                                v
                            ),
                        ]
                    }
                }
                (None, Some(a)) => {
                    if params.no_merge {
                        vec!["-f".to_string(), a.replace('+', ",")]
                    } else {
                        vec!["-f".to_string(), a.to_string()]
                    }
                }
                (None, None) if params.no_merge => {
                    vec![
                        "-f".to_string(),
                        "bestvideo/best[vcodec!=none],bestaudio".to_string(),
                    ]
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
    validate_time_range(params.start_time, params.end_time)?;
    let mut args: Vec<String> = vec![
        "--newline".to_string(),
        "--ignore-config".to_string(),  // 忽略用户系统配置，防止干扰 GUI
        "--color".to_string(), "never".to_string(),  // 禁用 ANSI 颜色转义序列
        // 使用 --progress-template 输出结构化进度
        // download 模板：下载阶段的进度（包含 fragment 信息以支持 HLS/DASH/直播流）
        "--progress-template".to_string(),
        r#"download:PROGRESS_JSON:{"percent":"%(progress._percent_str|0%)s","speed":"%(progress._speed_str|)s","eta":"%(progress._eta_str|)s","downloaded":"%(progress._downloaded_bytes_str|)s","total":"%(progress._total_bytes_str|)s","fragmentIndex":"%(progress.fragment_index|0)s","fragmentCount":"%(progress.fragment_count|0)s","status":"downloading"}"#.to_string(),
        // postprocess 模板：后处理阶段（合并/嵌入等），前端据此区分下载与后处理
        "--progress-template".to_string(),
        r#"postprocess:PROGRESS_JSON:{"status":"postprocessing"}"#.to_string(),
        // 进度输出间隔，避免高频刷屏
        "--progress-delta".to_string(),
        "1".to_string(),
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

    append_output_args(&mut args, params);
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
    // 封面存为单独文件（与 -o 同名模板不同扩展名），与嵌入封面可同时使用
    if params.write_thumbnail {
        args.push("--write-thumbnail".to_string());
    }
    // 简介存为单独的 .description 文件（同样跟随 -o 命名模板）
    if params.write_description {
        args.push("--write-description".to_string());
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
    if requires_audio_extraction(params) {
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
    // 重新混流 / 合并封装格式（无需二次编码，解决 Issue #51, #33, #30）
    if let Some(ref remux) = params.remux_format {
        if !remux.is_empty() && remux != "auto" {
            args.push("--remux-video".to_string());
            args.push(remux.clone());
            args.push("--merge-output-format".to_string());
            args.push(remux.clone());
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
    // 用户自定义 yt-dlp 附加参数（如 --sleep-requests、--sleep-interval 等，解决 Issue #50）
    if let Some(ref custom) = params.custom_args {
        if !custom.trim().is_empty() {
            let split_args = utils::split_command_line_args(custom)?;
            for arg in split_args {
                args.push(arg);
            }
        }
    }

    // 字幕
    append_subtitle_args(&mut args, &params.subtitles);

    // 时间范围裁剪（仅在有实际裁剪范围时添加，避免 *0-inf 触发不必要的 ffmpeg 处理）
    // 前端已将 time picker 值转换为秒数
    append_time_range_args(&mut args, params);

    // 播放列表
    if params.no_playlist {
        args.push("--no-playlist".to_string());
    } else if let Some(ref items) = params.playlist_items {
        if !items.is_empty() {
            args.push("--playlist-items".to_string());
            args.push(items.clone());
        }
    }

    // 直播流：从开始下载（实验性，仅 YouTube/Twitch/TVer/mellow-fan）
    if params.live_from_start {
        args.push("--live-from-start".to_string());
    }

    // URL（必须放在最后）
    args.push(params.url.clone());

    Ok(args)
}

fn append_time_range_args(args: &mut Vec<String>, params: &DownloadParams) {
    let has_start = params.start_time.is_some_and(|time| time > 0.0);
    if !has_start && params.end_time.is_none() {
        return;
    }
    let start = params.start_time.unwrap_or(0.0);
    let end = params
        .end_time
        .map(|time| time.to_string())
        .unwrap_or_else(|| "inf".to_string());
    args.extend(["--download-sections".to_string(), format!("*{start}-{end}")]);
    if params.force_keyframes_at_cuts {
        args.push("--force-keyframes-at-cuts".to_string());
    }
}

fn validate_time_range(start: Option<f64>, end: Option<f64>) -> Result<(), String> {
    let start = start.unwrap_or(0.0);
    if !start.is_finite() || start < 0.0 || end.is_some_and(|end| !end.is_finite() || end <= start)
    {
        return Err("Invalid time range: the end must be after a non-negative start".into());
    }
    Ok(())
}

#[cfg(test)]
mod time_range_tests {
    use super::{append_time_range_args, validate_time_range, DownloadParams};

    #[test]
    fn rejects_invalid_ranges_before_starting_ffmpeg() {
        assert!(validate_time_range(Some(60.0), Some(60.0)).is_err());
        assert!(validate_time_range(Some(-1.0), None).is_err());
        assert!(validate_time_range(Some(f64::NAN), None).is_err());
        assert!(validate_time_range(None, Some(f64::INFINITY)).is_err());
        assert!(validate_time_range(None, Some(20.0)).is_ok());
        assert!(validate_time_range(Some(60.0), None).is_ok());
    }

    #[test]
    fn precise_cuts_are_opt_in_and_only_apply_to_actual_sections() {
        let mut params = DownloadParams {
            start_time: Some(10.0),
            end_time: Some(30.0),
            ..Default::default()
        };
        let mut args = Vec::new();
        append_time_range_args(&mut args, &params);
        assert_eq!(args, ["--download-sections", "*10-30"]);
        params.force_keyframes_at_cuts = true;
        args.clear();
        append_time_range_args(&mut args, &params);
        assert_eq!(
            args,
            ["--download-sections", "*10-30", "--force-keyframes-at-cuts"]
        );

        params.end_time = None;
        args.clear();
        append_time_range_args(&mut args, &params);
        assert_eq!(
            args,
            [
                "--download-sections",
                "*10-inf",
                "--force-keyframes-at-cuts"
            ]
        );
        params.start_time = Some(0.0);
        args.clear();
        append_time_range_args(&mut args, &params);
        assert!(args.is_empty());
        params.start_time = None;
        append_time_range_args(&mut args, &params);
        assert!(args.is_empty());
        params.end_time = Some(20.0);
        append_time_range_args(&mut args, &params);
        assert_eq!(
            args,
            ["--download-sections", "*0-20", "--force-keyframes-at-cuts"]
        );
    }

    #[test]
    fn legacy_params_default_to_fast_cuts_and_camel_case_enables_precise_cuts() {
        let legacy: DownloadParams = serde_json::from_value(serde_json::json!({
            "startTime": 10,
            "endTime": 30
        }))
        .unwrap();
        assert!(!legacy.force_keyframes_at_cuts);
        let mut args = Vec::new();
        append_time_range_args(&mut args, &legacy);
        assert_eq!(args, ["--download-sections", "*10-30"]);

        let precise: DownloadParams = serde_json::from_value(serde_json::json!({
            "forceKeyframesAtCuts": true,
            "endTime": 30
        }))
        .unwrap();
        assert!(precise.force_keyframes_at_cuts);
        args.clear();
        append_time_range_args(&mut args, &precise);
        assert_eq!(
            args,
            ["--download-sections", "*0-30", "--force-keyframes-at-cuts"]
        );
    }
}

/// Keep the template relative so yt-dlp can honor a separate `-P temp:...` path.
fn append_output_args(args: &mut Vec<String>, params: &DownloadParams) {
    let template = params
        .output_template
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or("%(title).200s.%(ext)s");
    // Different selected streams can share an extension. A format suffix keeps
    // separate files from overwriting or being mistaken for an existing download.
    let template = if params.no_merge && params.download_mode == "default" {
        let base = template.strip_suffix(".%(ext)s").unwrap_or(template);
        format!("{base}.f%(format_id)s.%(ext)s")
    } else {
        template.to_string()
    };
    args.extend([
        "--paths".to_string(),
        format!("home:{}", params.download_dir),
        "-o".to_string(),
        template,
    ]);
}

#[cfg(test)]
mod output_arg_tests {
    use super::{append_output_args, DownloadParams};

    #[test]
    fn keeps_home_directory_separate_from_relative_template() {
        let params = DownloadParams {
            download_dir: r"E:\Downloads".into(),
            output_template: Some("%(playlist_title)s/%(title)s.%(ext)s".into()),
            ..Default::default()
        };
        let mut args = Vec::new();
        append_output_args(&mut args, &params);
        assert_eq!(
            args,
            [
                "--paths",
                r"home:E:\Downloads",
                "-o",
                "%(playlist_title)s/%(title)s.%(ext)s"
            ]
        );
    }

    #[test]
    fn empty_template_uses_relative_default() {
        let mut args = Vec::new();
        append_output_args(
            &mut args,
            &DownloadParams {
                download_dir: "/downloads".into(),
                output_template: Some(String::new()),
                ..Default::default()
            },
        );
        assert_eq!(
            args,
            ["--paths", "home:/downloads", "-o", "%(title).200s.%(ext)s"]
        );
    }

    #[test]
    fn separate_streams_get_distinct_names_inside_the_selected_directory() {
        let mut args = Vec::new();
        append_output_args(
            &mut args,
            &DownloadParams {
                download_dir: "/downloads".into(),
                download_mode: "default".into(),
                no_merge: true,
                output_template: Some("%(playlist_title)s/%(title)s.%(ext)s".into()),
                ..Default::default()
            },
        );
        assert_eq!(
            args,
            [
                "--paths",
                "home:/downloads",
                "-o",
                "%(playlist_title)s/%(title)s.f%(format_id)s.%(ext)s"
            ]
        );
    }
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
    use super::{build_format_args, requires_audio_extraction, requires_ffmpeg, DownloadParams};

    fn params() -> DownloadParams {
        DownloadParams {
            id: "test".to_string(),
            url: "https://example.com/video".to_string(),
            download_dir: ".".to_string(),
            download_mode: "default".to_string(),
            video_format: Some("137".to_string()),
            video_has_audio: false,
            audio_format: Some("140".to_string()),
            cookie_file: None,
            cookie_browser: None,
            proxy: None,
            output_template: None,
            concurrent_fragments: None,
            no_overwrites: false,
            embed_subs: false,
            embed_thumbnail: false,
            write_thumbnail: false,
            write_description: false,
            embed_metadata: false,
            embed_chapters: false,
            sponsorblock_remove: false,
            extract_audio: false,
            audio_convert_format: None,
            no_merge: false,
            recode_format: None,
            remux_format: None,
            limit_rate: None,
            ffmpeg_args: None,
            custom_args: None,
            subtitles: Vec::new(),
            start_time: None,
            end_time: None,
            force_keyframes_at_cuts: false,
            no_playlist: false,
            playlist_items: None,
            live_from_start: false,
        }
    }

    #[test]
    fn separate_video_and_audio_require_ffmpeg_for_default_download() {
        assert!(requires_ffmpeg(&params()));
    }

    #[test]
    fn combined_formats_can_download_directly_without_ffmpeg() {
        let mut value = params();
        value.video_has_audio = true;
        value.audio_format = None;
        assert!(!requires_ffmpeg(&value));
        assert_eq!(build_format_args(&value), ["-f", "137"]);
    }

    #[test]
    fn audio_only_sources_download_without_merging_or_requiring_ffmpeg() {
        let mut value = params();
        value.video_format = None;
        assert!(!requires_ffmpeg(&value));
        assert_eq!(build_format_args(&value), ["-f", "140"]);
        value.playlist_items = Some("1,2".into());
        assert_eq!(build_format_args(&value), ["-f", "140/bestaudio/best"]);
        value.playlist_items = None;
        value.audio_format = Some("140-en+140-es".into());
        assert!(requires_ffmpeg(&value));
        assert_eq!(
            build_format_args(&value),
            ["-f", "140-en+140-es", "--audio-multistreams"]
        );
        value.download_mode = "audio".into();
        assert!(!requires_ffmpeg(&value));
        assert_eq!(build_format_args(&value), ["-f", "140-en"]);
    }

    #[test]
    fn extraction_downloads_the_selected_audio_without_fetching_video() {
        let mut value = params();
        value.video_has_audio = true;
        value.extract_audio = true;
        assert_eq!(build_format_args(&value), ["-f", "140"]);
        value.audio_format = Some("140-en+140-es".into());
        assert_eq!(build_format_args(&value), ["-f", "140-en"]);
        value.audio_format = None;
        assert_eq!(build_format_args(&value), ["-f", "bestaudio/best"]);
        assert!(requires_ffmpeg(&value));
        value.download_mode = "video".into();
        assert_eq!(build_format_args(&value), ["-f", "bestaudio/best"]);
    }

    #[test]
    fn audio_only_mode_extracts_audio_when_only_combined_formats_exist() {
        let mut value = params();
        value.download_mode = "audio".into();
        value.audio_format = None;
        assert!(requires_ffmpeg(&value));
        assert!(requires_audio_extraction(&value));
        assert_eq!(build_format_args(&value), ["-f", "bestaudio/best"]);
    }

    #[test]
    fn explicit_no_merge_allows_separate_output_files() {
        let mut value = params();
        value.no_merge = true;
        assert!(!requires_ffmpeg(&value));
    }

    #[test]
    fn no_merge_downloads_video_and_audio_separately() {
        let mut value = params();
        value.no_merge = true;
        let args = build_format_args(&value);
        assert_eq!(args, ["-f", "137,140"]);
    }

    #[test]
    fn no_merge_without_selected_formats_never_uses_the_default_merged_selector() {
        let mut value = params();
        value.no_merge = true;
        value.video_format = None;
        value.audio_format = None;
        assert!(!requires_ffmpeg(&value));
        assert_eq!(
            build_format_args(&value),
            ["-f", "bestvideo/best[vcodec!=none],bestaudio"]
        );
        value.playlist_items = Some("1,2".into());
        assert_eq!(
            build_format_args(&value),
            ["-f", "bestvideo/best[vcodec!=none],bestaudio"]
        );
        value.video_format = Some(String::new());
        value.audio_format = Some(String::new());
        assert_eq!(
            build_format_args(&value),
            ["-f", "bestvideo/best[vcodec!=none],bestaudio"]
        );
    }

    #[test]
    fn no_merge_keeps_audio_extraction_and_single_stream_modes_separate() {
        let mut value = params();
        value.no_merge = true;
        value.video_format = None;
        value.audio_format = None;
        value.playlist_items = Some("1,2".into());
        value.extract_audio = true;
        assert!(requires_ffmpeg(&value));
        assert_eq!(build_format_args(&value), ["-f", "bestaudio/best"]);
        value.extract_audio = false;
        value.download_mode = "audio".into();
        assert!(requires_ffmpeg(&value));
        assert_eq!(build_format_args(&value), ["-f", "bestaudio/best"]);
        value.download_mode = "video".into();
        assert!(!requires_ffmpeg(&value));
        assert_eq!(build_format_args(&value), ["-f", "bestvideo/best"]);
    }

    #[test]
    fn merge_mode_uses_plus_concatenation() {
        let args = build_format_args(&params());
        assert_eq!(args, ["-f", "137+140"]);
    }

    #[test]
    fn multiple_audio_tracks_are_preserved_in_the_merged_video() {
        let mut value = params();
        value.audio_format = Some("140-en+140-es".into());
        assert_eq!(
            build_format_args(&value),
            ["-f", "137+140-en+140-es", "--audio-multistreams"]
        );
        value.download_mode = "video".into();
        assert_eq!(build_format_args(&value), ["-f", "137"]);
        value.download_mode = "default".into();
        value.no_merge = true;
        assert_eq!(build_format_args(&value), ["-f", "137,140-en,140-es"]);
    }

    #[test]
    fn selected_audio_is_not_dropped_when_video_already_has_a_soundtrack() {
        let mut value = params();
        value.video_has_audio = true;
        assert_eq!(
            build_format_args(&value),
            ["-f", "137+140", "--audio-multistreams"]
        );
        value.no_merge = true;
        assert_eq!(build_format_args(&value), ["-f", "137,140"]);
    }

    #[test]
    fn separate_playlist_formats_each_have_their_own_fallback() {
        let mut value = params();
        value.no_merge = true;
        value.playlist_items = Some("1,2".into());
        value.audio_format = Some("140-en+140-es".into());
        assert_eq!(
            build_format_args(&value),
            [
                "-f",
                "137/bestvideo/best,140-en/bestaudio/best,140-es/bestaudio/best"
            ]
        );
        value.video_format = None;
        assert_eq!(
            build_format_args(&value),
            ["-f", "140-en/bestaudio/best,140-es/bestaudio/best"]
        );
    }

    #[test]
    fn playlist_entries_can_fall_back_when_format_ids_differ() {
        let mut value = params();
        value.playlist_items = Some("1,2".into());
        assert_eq!(
            build_format_args(&value),
            ["-f", "137+140/bestvideo*+bestaudio/best"]
        );
        value.download_mode = "audio".into();
        assert_eq!(build_format_args(&value), ["-f", "140/bestaudio"]);
        value.no_playlist = true;
        assert_eq!(build_format_args(&value), ["-f", "140"]);
    }

    #[test]
    fn single_video_selection_does_not_fall_back_to_a_silent_video() {
        let mut value = params();
        value.audio_format = None;
        assert_eq!(
            build_format_args(&value),
            [
                "-f",
                "137[acodec!=none]/137+bestaudio/best[vcodec!=none][acodec!=none]"
            ]
        );
        assert!(requires_ffmpeg(&value));
    }

    #[test]
    fn postprocessing_requires_ffmpeg_even_without_a_merge() {
        let mut value = params();
        value.no_merge = true;
        value.extract_audio = true;
        assert!(requires_ffmpeg(&value));
        value.extract_audio = false;
        value.start_time = Some(15.0);
        assert!(requires_ffmpeg(&value));
        value.start_time = None;
        value.remux_format = Some("mkv".into());
        assert!(requires_ffmpeg(&value));
    }
}
