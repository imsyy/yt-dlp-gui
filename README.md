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
- **Plugin Manager** — install yt-dlp plugins (e.g. ChromeCookieUnlock) with one click
- **Browser Extension** — companion Chrome / Edge extension that sends the page URL and required cookies to the app with one click ([details](#browser-extension))

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

| Home (Dark) | Home (Light) |
|:-:|:-:|
| ![Home](screenshot/home.png) | ![Home Light](screenshot/home-light.png) |

| Download Options | Extra Options |
|:-:|:-:|
| ![Download](screenshot/download.png) | ![Download Other](screenshot/download-other.png) |

| Downloading | Tools |
|:-:|:-:|
| ![Downloading](screenshot/downloading.png) | ![Tools](screenshot/tools.png) |

## Getting Started

### Download

Grab the latest release for your platform from [**Releases**](https://github.com/imsyy/yt-dlp-gui/releases):

| Platform | File |
|----------|------|
| Windows  | `.exe` installer |
| macOS    | `.dmg` |
| Linux    | `.AppImage` / `.deb` / `.rpm` |

#### Arch Linux

Use the x86_64 AppImage from the release page. AppImage uses FUSE 2 on Arch, so install the compatibility package once, make the download executable, and run it:

```bash
sudo pacman -S fuse2
chmod +x YDL.GUI_*.AppImage
./YDL.GUI_*.AppImage
```

If FUSE cannot be enabled in your environment, run the same file with `--appimage-extract-and-run` instead. The AppImage is portable and does not require a system-wide package installation.

### First Launch

1. Open the app and go to **Settings**
2. Click **Download** next to yt-dlp — the binary is fetched automatically
3. *(Optional)* Install **Deno** runtime for full YouTube format support
4. Set your **download directory**
5. Go back to the home page, paste a URL, and start downloading

> [!TIP]
> If you encounter login-required videos, configure Cookie in settings using Netscape format text or a cookie file.

## Browser Extension

A companion **YDL GUI Helper** browser extension lives in [`browser-extension/`](./browser-extension/). It sends the current tab's URL and required cookies straight to the desktop app via a local protocol handler (`ytdlp-gui://`) — no copy-paste, no extra cookie export.

### Highlights

- One-click send from the popup, or right-click context menu (`Send page to YDL GUI` / `Download link with YDL GUI` / `Send selected URL to YDL GUI`)
- Action badge lights up automatically on supported video sites
- Auto light / dark theme that follows your system
- Cookies are processed locally — passed straight to the app via the local protocol, never uploaded anywhere

### Install (Chrome / Edge / Brave / Vivaldi etc.)

The extension is bundled with the app — no separate download required.

1. In the app, open **Toolbox → Browser Extension** and click **Open extension folder**.
2. Open `chrome://extensions` (or `edge://extensions`) and turn on **Developer mode** in the top-right.
3. Click **Load unpacked** and select the folder revealed in step 1.
4. Pin the YDL GUI Helper icon next to the address bar.

### Use

1. Open a supported video page (YouTube, Bilibili, Twitch, Vimeo, Twitter/X, TikTok, Instagram, Facebook, Reddit, SoundCloud, etc.).
2. Click the YDL GUI icon, or right-click the page / a video link and choose **Send to YDL GUI**.
3. The desktop app comes to the front automatically with the URL and cookies pre-filled.

> [!NOTE]
> Make sure the YDL GUI desktop app is installed and running for the protocol handler to fire.

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
- [pnpm](https://pnpm.io/) >= 11
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
