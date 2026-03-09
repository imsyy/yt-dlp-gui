<div align="center">

<img src="./public/app-icon.svg" width="80" height="80" alt="yt-dlp GUI">

# yt-dlp GUI

现代化、美观的 [yt-dlp](https://github.com/yt-dlp/yt-dlp) 桌面客户端。

支持 YouTube、Bilibili、Twitter/X 等 [1000+ 网站](https://github.com/yt-dlp/yt-dlp/blob/master/supportedsites.md)的视频下载。

[![License](https://img.shields.io/github/license/imsyy/yt-dlp-gui?color=f0f0f0&labelColor=555555)](LICENSE)
[![Release](https://img.shields.io/github/v/release/imsyy/yt-dlp-gui?color=f0f0f0&labelColor=555555)](https://github.com/imsyy/yt-dlp-gui/releases)
[![Stars](https://img.shields.io/github/stars/imsyy/yt-dlp-gui?style=flat&color=f0f0f0&labelColor=555555)](https://github.com/imsyy/yt-dlp-gui)
[![Downloads](https://img.shields.io/github/downloads/imsyy/yt-dlp-gui/total?color=f0f0f0&labelColor=555555)](https://github.com/imsyy/yt-dlp-gui/releases)

[English](./README.md) | **简体中文**

</div>

---

## 为什么选择 yt-dlp GUI？

yt-dlp 功能强大，但命令行操作门槛较高。**yt-dlp GUI** 将其封装为简洁的原生桌面应用 — 无需终端，开箱即用。

- **零配置上手** — 粘贴链接、选择画质、一键下载
- **原生轻量** — 基于 Tauri 2 + Rust 构建，安装包约 10 MB，内存占用极低
- **跨平台** — 支持 Windows、macOS、Linux
- **多语言** — 支持 7 种语言，自动匹配系统语言

## 功能特性

### 核心功能

- 粘贴视频链接，即刻预览标题、封面、时长及可用格式
- 自由选择画质，支持仅视频、仅音频下载
- 下载队列管理，支持暂停 / 继续 / 取消
- 实时进度显示，包含下载速度和预估剩余时间
- 播放列表支持 — 批量下载或选择指定视频
- 可配置并发下载数和分片线程数

### 工具箱

- **封面下载** — 浏览并保存所有可用分辨率的封面图片
- **字幕提取** — 下载 SRT / VTT / ASS / LRC 格式字幕，支持双语字幕合成
- **直播弹幕归档** — 提取 YouTube 直播回放弹幕，支持正则筛选，导出为 JSON / CSV

### 进阶功能

- 自定义文件名模板，支持丰富变量（标题、作者、日期、分辨率等）
- 时间裁剪 — 仅下载视频的指定片段
- 格式转换：MP4 / MKV / WebM / MP3 / AAC / FLAC 等
- 嵌入字幕、缩略图、元数据、章节到输出文件
- SponsorBlock 集成 — 自动跳过赞助片段
- Cookie 身份验证，支持年龄限制和会员专属内容
- 代理支持（HTTP / SOCKS）
- 下载限速
- 亮色 / 暗色 / 跟随系统主题
- 下载完成通知（应用内 和/或 系统通知）

## 截图

| 首页（暗色） | 首页（亮色） |
|:-:|:-:|
| ![Home](screenshot/home.png) | ![Home Light](screenshot/home-light.png) |

| 下载选项 | 更多选项 |
|:-:|:-:|
| ![Download](screenshot/download.png) | ![Download Other](screenshot/download-other.png) |

| 下载中 | 工具箱 |
|:-:|:-:|
| ![Downloading](screenshot/downloading.png) | ![Tools](screenshot/tools.png) |

## 快速开始

### 下载安装

前往 [**Releases**](https://github.com/imsyy/yt-dlp-gui/releases) 下载对应平台的安装包：

| 平台 | 文件 |
|------|------|
| Windows | `.exe` 安装包 |
| macOS | `.dmg` |
| Linux | `.AppImage` / `.deb` |

### 首次使用

1. 打开应用，进入**设置**页面
2. 点击 yt-dlp 旁的**下载**按钮 — 程序会自动获取最新版本
3. *（可选）* 安装 **Deno** 运行时以获取完整的 YouTube 格式列表
4. 设置**下载目录**
5. 返回首页，粘贴视频链接即可开始下载

> [!TIP]
> 如果遇到需要登录的视频，请在设置中配置 Cookie（支持 Netscape 格式文本或 Cookie 文件）。

## 技术栈

| 层级 | 技术 |
|------|------|
| 后端 | [Tauri 2](https://tauri.app/) + [Rust](https://www.rust-lang.org/) |
| 前端 | [Vue 3](https://vuejs.org/) + [TypeScript](https://www.typescriptlang.org/) |
| UI 组件 | [Naive UI](https://www.naiveui.com/) |
| 状态管理 | [Pinia](https://pinia.vuejs.org/) + 持久化 |
| 构建工具 | [Vite](https://vitejs.dev/) |
| 国际化 | [Vue I18n](https://vue-i18n.intlify.dev/) — zh-CN、zh-TW、en-US、ja-JP、ko-KR、es-ES、ru-RU |

## 开发

### 环境要求

- [Node.js](https://nodejs.org/) >= 22
- [pnpm](https://pnpm.io/)
- [Rust](https://www.rust-lang.org/tools/install)

### 开始开发

```bash
# 克隆仓库
git clone https://github.com/imsyy/yt-dlp-gui.git
cd yt-dlp-gui

# 安装依赖
pnpm install

# 开发模式运行（Vite + Tauri）
pnpm tauri:dev

# 构建生产版本
pnpm tauri:build
```

## 参与贡献

欢迎提交 [Issue](https://github.com/imsyy/yt-dlp-gui/issues) 或 Pull Request。

## 开源协议

[MIT](LICENSE) &copy; [imsyy](https://github.com/imsyy)
