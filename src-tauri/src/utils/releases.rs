//! 各平台外部工具的官方发行下载地址。

use super::source::get_ytdlp_channel;

/// 获取工具的最新稳定发行页；请求完成后的重定向 URL 包含实际版本标签。
/// yt-dlp 按当前所选通道返回对应仓库地址（stable/nightly/master）。
pub fn get_tool_latest_release_url(tool: &str) -> Option<String> {
    match tool {
        "yt-dlp" => Some(get_ytdlp_latest_release_url()),
        "deno" => Some("https://github.com/denoland/deno/releases/latest".to_string()),
        "ffmpeg" => Some("https://github.com/eugeneware/ffmpeg-static/releases/latest".to_string()),
        _ => None,
    }
}

/// 当前通道下 yt-dlp 最新发行页；重定向 URL 包含实际版本标签。
pub fn get_ytdlp_latest_release_url() -> String {
    format!(
        "https://github.com/{}/releases/latest",
        get_ytdlp_channel().repository()
    )
}

/// 当前平台下 yt-dlp 可执行文件在对应通道中的资产名。
fn ytdlp_asset_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "yt-dlp.exe"
    } else if cfg!(target_os = "macos") {
        "yt-dlp_macos"
    } else {
        "yt-dlp_linux"
    }
}

/// 获取指定通道的 yt-dlp 下载地址（根据平台）。
/// 三个通道的构建产物使用相同的资产文件名。
pub fn get_ytdlp_download_url_for_channel(channel: &str) -> Result<String, String> {
    let channel = channel.trim();
    let repository = match channel {
        "stable" => "yt-dlp/yt-dlp",
        "nightly" => "yt-dlp/yt-dlp-nightly-builds",
        "master" => "yt-dlp/yt-dlp-master-builds",
        _ => return Err(format!("err_invalid_ytdlp_channel:{}", channel)),
    };
    Ok(format!(
        "https://github.com/{}/releases/latest/download/{}",
        repository,
        ytdlp_asset_name()
    ))
}

/// 获取当前所选通道的 yt-dlp 下载地址（根据平台）。
pub fn get_ytdlp_download_url() -> String {
    get_ytdlp_download_url_for_channel(get_ytdlp_channel().as_str()).unwrap_or_else(|_| {
            format!(
                "https://github.com/yt-dlp/yt-dlp/releases/latest/download/{}",
                ytdlp_asset_name()
            )
        })
}

/// 获取 Deno 下载地址（根据平台和架构）
pub fn get_deno_download_url() -> &'static str {
    if cfg!(target_os = "windows") {
        if cfg!(target_arch = "aarch64") {
            "https://github.com/denoland/deno/releases/latest/download/deno-aarch64-pc-windows-msvc.zip"
        } else {
            "https://github.com/denoland/deno/releases/latest/download/deno-x86_64-pc-windows-msvc.zip"
        }
    } else if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") {
            "https://github.com/denoland/deno/releases/latest/download/deno-aarch64-apple-darwin.zip"
        } else {
            "https://github.com/denoland/deno/releases/latest/download/deno-x86_64-apple-darwin.zip"
        }
    } else if cfg!(target_arch = "aarch64") {
        "https://github.com/denoland/deno/releases/latest/download/deno-aarch64-unknown-linux-gnu.zip"
    } else {
        "https://github.com/denoland/deno/releases/latest/download/deno-x86_64-unknown-linux-gnu.zip"
    }
}

pub fn get_ffmpeg_download_urls() -> [(&'static str, &'static str); 2] {
    let platform = if cfg!(target_os = "windows") {
        "win32-x64"
    } else if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") {
            "darwin-arm64"
        } else {
            "darwin-x64"
        }
    } else if cfg!(target_arch = "aarch64") {
        "linux-arm64"
    } else if cfg!(target_arch = "arm") {
        "linux-arm"
    } else {
        "linux-x64"
    };

    match platform {
        "darwin-arm64" => [
            ("ffmpeg", "https://github.com/eugeneware/ffmpeg-static/releases/latest/download/ffmpeg-darwin-arm64"),
            ("ffprobe", "https://github.com/eugeneware/ffmpeg-static/releases/latest/download/ffprobe-darwin-arm64"),
        ],
        "darwin-x64" => [
            ("ffmpeg", "https://github.com/eugeneware/ffmpeg-static/releases/latest/download/ffmpeg-darwin-x64"),
            ("ffprobe", "https://github.com/eugeneware/ffmpeg-static/releases/latest/download/ffprobe-darwin-x64"),
        ],
        "linux-arm64" => [
            ("ffmpeg", "https://github.com/eugeneware/ffmpeg-static/releases/latest/download/ffmpeg-linux-arm64"),
            ("ffprobe", "https://github.com/eugeneware/ffmpeg-static/releases/latest/download/ffprobe-linux-arm64"),
        ],
        "linux-arm" => [
            ("ffmpeg", "https://github.com/eugeneware/ffmpeg-static/releases/latest/download/ffmpeg-linux-arm"),
            ("ffprobe", "https://github.com/eugeneware/ffmpeg-static/releases/latest/download/ffprobe-linux-arm"),
        ],
        "win32-x64" => [
            ("ffmpeg", "https://github.com/eugeneware/ffmpeg-static/releases/latest/download/ffmpeg-win32-x64"),
            ("ffprobe", "https://github.com/eugeneware/ffmpeg-static/releases/latest/download/ffprobe-win32-x64"),
        ],
        _ => [
            ("ffmpeg", "https://github.com/eugeneware/ffmpeg-static/releases/latest/download/ffmpeg-linux-x64"),
            ("ffprobe", "https://github.com/eugeneware/ffmpeg-static/releases/latest/download/ffprobe-linux-x64"),
        ],
    }
}
