//! 各平台外部工具的官方发行下载地址。

/// 获取工具的最新稳定发行页；请求完成后的重定向 URL 包含实际版本标签。
pub fn get_tool_latest_release_url(tool: &str) -> Option<&'static str> {
    match tool {
        "yt-dlp" => Some("https://github.com/yt-dlp/yt-dlp/releases/latest"),
        "deno" => Some("https://github.com/denoland/deno/releases/latest"),
        "ffmpeg" => Some("https://github.com/eugeneware/ffmpeg-static/releases/latest"),
        _ => None,
    }
}

/// 获取 yt-dlp 下载地址（根据平台）
pub fn get_ytdlp_download_url() -> &'static str {
    if cfg!(target_os = "windows") {
        "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe"
    } else if cfg!(target_os = "macos") {
        "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_macos"
    } else {
        "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux"
    }
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
