<div align="center">

<img src="./public/app-icon.svg" width="80" height="80" alt="yt-dlp GUI">

# yt-dlp GUI

A modern, beautiful desktop GUI for [yt-dlp](https://github.com/yt-dlp/yt-dlp).

Download videos from YouTube, Bilibili, Twitter/X and [1000+ websites](https://github.com/yt-dlp/yt-dlp/blob/master/supportedsites.md) with ease.

[![License](https://img.shields.io/github/license/imsyy/yt-dlp-gui?color=f0f0f0&labelColor=555555)](LICENSE)
[![Release](https://img.shields.io/github/v/release/imsyy/yt-dlp-gui?color=f0f0f0&labelColor=555555)](https://github.com/imsyy/yt-dlp-gui/releases)
[![Stars](https://img.shields.io/github/stars/imsyy/yt-dlp-gui?style=flat&color=f0f0f0&labelColor=555555)](https://github.com/imsyy/yt-dlp-gui)
[![Downloads](https://img.shields.io/github/downloads/imsyy/yt-dlp-gui/total?color=f0f0f0&labelColor=555555)](https://github.com/imsyy/yt-dlp-gui/releases)

**English** | [简体中文](./README.zh-CN.md)

</div>

---

## Why yt-dlp GUI?

yt-dlp is powerful, but its command-line interface can be intimidating. **yt-dlp GUI** wraps it in a clean, native desktop app — no terminal needed.

- **Zero config to start** — paste a link, pick a quality, click download
- **Native & lightweight** — built with Tauri 2 + Rust, ~10 MB installer, low memory usage
- **Cross-platform** — Windows, macOS, and Linux
- **Multilingual** — 7 languages with auto-detection

## Features

### Core

- Paste a video URL and instantly preview title, thumbnail, duration, and available formats
- Choose video quality, audio-only, or video-only downloads
- Download queue with pause / resume / cancel controls
- Real-time progress with speed and ETA display
- Playlist support — download all or selected items
- Configurable concurrent downloads and fragment threading

### Toolbox

- **Thumbnail Downloader** — browse and save all available cover images in any resolution
- **Subtitle Extractor** — download subtitles in SRT / VTT / ASS / LRC, with bilingual merge support
- **Live Chat Archiver** — extract YouTube live chat replay, filter with regex, export as JSON / CSV

### Advanced

- Custom filename templates with rich variables (title, author, date, resolution, etc.)
- Time-based clip trimming — download only a segment of the video
- Re-encode to MP4 / MKV / WebM / MP3 / AAC / FLAC and more
- Embed subtitles, thumbnails, metadata, and chapters into the output file
- SponsorBlock integration — automatically skip sponsored segments
- Cookie authentication for age-restricted or members-only content
- Proxy support (HTTP / SOCKS)
- Download speed limiter
- Light / Dark / Auto theme
- Download completion notifications (in-app and/or system)

## Screenshots

> Coming soon

## Getting Started

### Download

Grab the latest release for your platform from [**Releases**](https://github.com/imsyy/yt-dlp-gui/releases):

| Platform | File |
|----------|------|
| Windows  | `.exe` installer |
| macOS    | `.dmg` |
| Linux    | `.AppImage` / `.deb` |

### First Launch

1. Open the app and go to **Settings**
2. Click **Download** next to yt-dlp — the binary is fetched automatically
3. *(Optional)* Install **Deno** runtime for full YouTube format support
4. Set your **download directory**
5. Go back to the home page, paste a URL, and start downloading

> [!TIP]
> If you encounter login-required videos, configure Cookie in settings using Netscape format text or a cookie file.

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Backend | [Tauri 2](https://tauri.app/) + [Rust](https://www.rust-lang.org/) |
| Frontend | [Vue 3](https://vuejs.org/) + [TypeScript](https://www.typescriptlang.org/) |
| UI | [Naive UI](https://www.naiveui.com/) |
| State | [Pinia](https://pinia.vuejs.org/) with persistence |
| Build | [Vite](https://vitejs.dev/) |
| i18n | [Vue I18n](https://vue-i18n.intlify.dev/) — zh-CN, zh-TW, en-US, ja-JP, ko-KR, es-ES, ru-RU |

## Development

### Prerequisites

- [Node.js](https://nodejs.org/) >= 22
- [pnpm](https://pnpm.io/)
- [Rust](https://www.rust-lang.org/tools/install)

### Setup

```bash
# Clone the repository
git clone https://github.com/imsyy/yt-dlp-gui.git
cd yt-dlp-gui

# Install dependencies
pnpm install

# Run in development mode (Vite + Tauri)
pnpm tauri:dev

# Build for production
pnpm tauri:build
```

## Contributing

Contributions are welcome! Feel free to open an [issue](https://github.com/imsyy/yt-dlp-gui/issues) or submit a pull request.

## License

[MIT](LICENSE) &copy; [imsyy](https://github.com/imsyy)
