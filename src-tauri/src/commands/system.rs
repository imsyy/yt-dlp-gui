//! 系统相关命令

use tauri::AppHandle;

/// 检测平台默认下载目录
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
