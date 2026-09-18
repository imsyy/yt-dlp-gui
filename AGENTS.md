# Repository Instructions

## Project Overview

yt-dlp-gui is a desktop video downloader built with Tauri 2, Rust, Vue 3, and TypeScript. The frontend is under `src/`; the Rust backend is under `src-tauri/src/`.

## Development Commands

```bash
pnpm install
pnpm dev
pnpm build
pnpm tauri dev
pnpm tauri build
pnpm test -- --run
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
```

Run focused checks while developing, then run the relevant frontend and Rust checks before handing off changes.

## Frontend Conventions

- Use Vue 3 `<script setup lang="ts">` single-file components and the `@` alias for `src/` paths.
- The project uses `unplugin-auto-import` and `unplugin-vue-components`, configured in `vite.config.ts`.
- Vue, Vue Router, VueUse, and configured Naive UI APIs are automatically imported. Check `auto-imports.d.ts` before adding an explicit API import.
- Components discovered by the component resolver are automatically imported. Check `components.d.ts` before adding an explicit component import.
- Do not explicitly import an API or component that is already covered by automatic imports. For example, use `<ToolUrlInput />` directly when it appears in `components.d.ts`; do not add `import ToolUrlInput from "@/components/toolbox/ToolUrlInput.vue"`.
- Treat `auto-imports.d.ts` and `components.d.ts` as generated declarations. Do not manually maintain entries that the Vite plugins generate.
- Use Pinia for shared application state. Do not persist component-local state unless the feature explicitly requires restoration across navigation or application restarts.
- Frontend-to-backend calls use Tauri `invoke` from `@tauri-apps/api/core`.

## Rust and Tauri Conventions

- Register Tauri commands in `src-tauri/src/lib.rs`.
- Keep command implementations grouped by functional domain under `src-tauri/src/commands/` and database code under `src-tauri/src/db/`.
- Prefer a single current-state record over an accumulating history table when a feature only needs to restore its latest state.
- Run `cargo check` and the relevant Rust tests after changing commands, migrations, or database models.
- Avoid formatting unrelated Rust files; preserve unrelated user changes in the working tree.

## Change Discipline

- Inspect the existing architecture and generated declarations before introducing helpers, imports, persistence, or abstractions.
- Keep changes scoped to the requested behavior and remove superseded commands, tables, types, and comments.
- Do not conflate transient/current UI state with user-facing history. History must be an explicit product requirement.
- Preserve existing unrelated worktree changes.

