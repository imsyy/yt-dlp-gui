# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

yt-dlp-gui is a desktop application for downloading videos via yt-dlp. Built with **Tauri 2** (Rust backend) + **Vue 3** (TypeScript frontend). The UI is in Chinese.

## Development Commands

```bash
pnpm install          # Install frontend dependencies
pnpm tauri dev        # Run the full app in development (starts Vite + Rust backend)
pnpm dev              # Run frontend only (Vite dev server on port 1420)
pnpm build            # Type-check and build frontend (vue-tsc + vite build)
pnpm tauri build      # Build production app bundle
```

Rust backend builds are handled by Tauri automatically during `pnpm tauri dev` / `pnpm tauri build`. To check Rust code independently:
```bash
cd src-tauri && cargo check
```

## Architecture

### Frontend (`src/`)
- **Vue 3 + TypeScript** with `<script setup>` SFCs
- **Naive UI** component library, auto-imported via `unplugin-vue-components` (NaiveUiResolver)
- **Auto-imports** configured in `vite.config.ts`: Vue, Vue Router, VueUse APIs, and Naive UI composables are available without explicit imports
- **Pinia** for state with `pinia-plugin-persistedstate` for localStorage persistence
- **Path alias**: `@` maps to `src/`
- **Pages**: Home (video search/download UI), Downloads, Settings
- **Tauri IPC**: Frontend calls Rust commands via `invoke()` from `@tauri-apps/api/core`

### Backend (`src-tauri/src/`)
- `lib.rs` — Tauri app builder, registers all commands and plugins
- `commands.rs` — All `#[tauri::command]` handlers: yt-dlp/Deno status, download, update, cookie management, video info fetching
- `utils.rs` — Path helpers (yt-dlp, Deno, cookie paths in app data dir), platform-specific download URLs, JS runtime args builder
- Binaries (yt-dlp, Deno) are downloaded to the Tauri app data directory at runtime, not bundled
- Progress events emitted to frontend via `app.emit()` (e.g., `ytdlp-download-progress`, `deno-download-progress`)

### Frontend-Backend Communication
- Tauri commands are invoked from Vue via `invoke<T>("command_name", { args })`
- Real-time progress uses Tauri event system (`app.emit` on Rust side)
- Shared types in `src/types/index.ts` mirror Rust structs in `commands.rs`

## Key Conventions

- Windows builds use `CREATE_NO_WINDOW` flag (0x08000000) on all subprocess spawns to hide console windows
- All yt-dlp commands set `PYTHONUTF8=1` environment variable
- Deno is optional — used as JS runtime for yt-dlp when installed (`--js-runtimes` flag)
- Cookie support: text (Netscape format saved to file) or direct file path
