# Auto-Detect Download Directory Feature

## Overview

The auto-detect download directory feature automatically detects the platform's default download folder for Windows, Linux, and macOS. This feature helps users quickly set up their download location without manually browsing for folders.

## Features

### Platform-Specific Detection

- **Windows**: Uses Tauri's `download_dir()` API to detect the system Downloads folder
- **macOS**: Uses `$HOME/Downloads` as the default location
- **Linux**: 
  - First checks `XDG_DOWNLOAD_DIR` environment variable
  - Falls back to `$HOME/Downloads` if not set

### Implementation

#### Backend (Rust)

Located in `src-tauri/src/commands/system.rs`:

```rust
#[tauri::command]
pub fn get_default_download_dir(app: AppHandle) -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        use std::path::PathBuf;
        let downloads = app
            .path()
            .download_dir()
            .map_err(|e| format!("Failed to get download directory: {}", e))?;
        Ok(downloads.to_string_lossy().to_string())
    }

    #[cfg(target_os = "macos")]
    {
        use std::path::PathBuf;
        let _ = app;
        let home = std::env::var("HOME").map_err(|e| format!("Failed to get HOME: {}", e))?;
        let downloads = PathBuf::from(home).join("Downloads");
        Ok(downloads.to_string_lossy().to_string())
    }

    #[cfg(target_os = "linux")]
    {
        use std::path::PathBuf;
        let _ = app;
        // Try XDG_DOWNLOAD_DIR first
        if let Ok(xdg) = std::env::var("XDG_DOWNLOAD_DIR") {
            return Ok(xdg);
        }
        // Fallback to HOME/Downloads
        let home = std::env::var("HOME").map_err(|e| format!("Failed to get HOME: {}", e))?;
        let downloads = PathBuf::from(home).join("Downloads");
        Ok(downloads.to_string_lossy().to_string())
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        let _ = app;
        Err("Unsupported platform".to_string())
    }
}
```

#### Frontend Integration

The auto-detect is automatically called during app bootstrap in `src/composables/useAppBootstrap.ts`:

```typescript
const autoDetectDownloadDir = async (): Promise<void> => {
  if (settingStore.downloadDir) return;
  try {
    const defaultDir = await invoke<string>("get_default_download_dir");
    settingStore.downloadDir = defaultDir;
  } catch {
    // 静默失败，用户可以手动设置
  }
};
```

This is invoked in the `bootstrap()` function, which runs on app startup:

```typescript
const bootstrap = async (): Promise<void> => {
  // ... other setup
  await autoDetectDownloadDir();
  // ... other setup
};
```

### Manual Auto-Detect

Users can also manually trigger auto-detect from the download directory settings card in `src/components/DownloadDirCard.vue`:

```typescript
const handleAutoDetect = async () => {
  try {
    const defaultDir = await invoke<string>("get_default_download_dir");
    settingStore.downloadDir = defaultDir;
    window.$message.success(t("downloadDir.autoDetectSuccess"));
  } catch (e: unknown) {
    window.$message.error(t("downloadDir.autoDetectFailed", { e }));
  }
};
```

### Quick Download Mode

When using quick download mode without a configured download directory, the system automatically attempts to detect the default download directory in `src/pages/Home.vue`:

```typescript
const ensureQuickDownloadConfigured = async (): Promise<boolean> => {
  if (settingStore.homeDownloadBehavior !== "quick" || settingStore.downloadDir) return true;

  // 尝试自动检测默认下载目录
  try {
    const defaultDir = await invoke<string>("get_default_download_dir");
    settingStore.downloadDir = defaultDir;
    window.$message.success(translate("downloadDir.autoDetectSuccess"));
    return true;
  } catch (e: unknown) {
    window.$message.warning(translate("detail.setDownloadDirFirst"));
    showQuickSettings.value = true;
    return false;
  }
};
```

## Behavior

1. **First Launch**: On first launch, if no download directory is configured, the app automatically detects and sets the platform's default Downloads folder
2. **Subsequent Launches**: If a download directory is already configured, the auto-detect is skipped
3. **Manual Override**: Users can manually change the download directory at any time via the settings page
4. **Silent Failure**: If auto-detect fails (e.g., unsupported platform or permission issues), the app continues to function and users can manually set the directory

## Localization

The feature includes localization strings in both English and Chinese:

**English (`src/locales/en-US/settings.json`)**:
```json
{
  "downloadDir": {
    "title": "Download Directory",
    "autoDetect": "Auto Detect",
    "autoDetectSuccess": "Download directory auto-detected successfully",
    "autoDetectFailed": "Failed to auto-detect download directory: {e}"
  }
}
```

**Chinese (`src/locales/zh-CN/settings.json`)**:
```json
{
  "downloadDir": {
    "title": "下载目录",
    "autoDetect": "自动检测",
    "autoDetectSuccess": "下载目录自动检测成功",
    "autoDetectFailed": "自动检测下载目录失败：{e}"
  }
}
```

## Testing

To test the auto-detect feature:

1. Clear the download directory setting (either by clearing localStorage or using the settings UI)
2. Restart the application
3. Verify that the download directory is set to the platform's default Downloads folder
4. Test on different platforms (Windows, macOS, Linux) to ensure platform-specific behavior

## Troubleshooting

- **Windows**: Ensure the app has permission to access the Downloads folder
- **Linux**: Check that `XDG_DOWNLOAD_DIR` is set correctly in `~/.config/user-dirs.dirs` if using a custom download location
- **macOS**: Verify that the HOME environment variable is set correctly
- **Permissions**: On some systems, the app may need additional permissions to access user directories
