//! yt-dlp 插件包的管理（扫描、导入、移除、打开目录）。

use crate::utils;
use std::path::{Path, PathBuf};
use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

/// 插件目录下的插件包项
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginItem {
    /// 文件名，如 `chrome-cookie-unlock.zip`
    pub file_name: String,
    /// 展示用名称（去掉扩展名）
    pub name: String,
    /// 文件大小（字节）
    pub size_bytes: u64,
    /// 修改时间（毫秒时间戳）
    pub modified_at: i64,
    /// 插件类型列表（"extractor", "postprocessor"）
    pub plugin_types: Vec<String>,
}

/// 列出插件目录下所有已导入的有效插件包
#[tauri::command]
pub async fn list_plugins(app: AppHandle) -> Result<Vec<PluginItem>, String> {
    let dir = utils::get_plugin_dir(&app)?;
    tokio::task::spawn_blocking(move || scan_plugins(&dir))
        .await
        .map_err(|e| format!("err_task:{}", e))?
}

/// 从本地选择的文件导入插件包（导入时校验是否包含 yt_dlp_plugins 结构）
#[tauri::command]
pub async fn import_plugin(app: AppHandle, source_path: String) -> Result<PluginItem, String> {
    let dir = utils::get_plugin_dir(&app)?;
    tokio::task::spawn_blocking(move || {
        let source = PathBuf::from(&source_path);
        if !source.is_file() {
            return Err("err_file_not_found".to_string());
        }

        let file_name = source
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or("err_invalid_plugin_name")?;

        if !file_name.to_ascii_lowercase().ends_with(".zip") {
            return Err("err_plugin_layout".to_string());
        }

        // 预检：必须是合法的插件 zip（包含 yt_dlp_plugins 结构），否则直接拒绝
        let types = inspect_plugin_types(&source)?;

        std::fs::create_dir_all(&dir).map_err(|e| format!("err_create_dir:{}", e))?;
        let target = dir.join(file_name);
        std::fs::copy(&source, &target).map_err(|e| format!("err_save_plugin:{}", e))?;

        let metadata = std::fs::metadata(&target).map_err(|e| format!("err_read_plugin:{}", e))?;
        let name = target
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or(file_name)
            .to_string();

        let modified_at = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|duration| duration.as_millis() as i64)
            .unwrap_or_default();

        Ok(PluginItem {
            file_name: file_name.to_string(),
            name,
            size_bytes: metadata.len(),
            modified_at,
            plugin_types: types,
        })
    })
    .await
    .map_err(|e| format!("err_task:{}", e))?
}

/// 移除插件包
#[tauri::command]
pub async fn remove_plugin(app: AppHandle, file_name: String) -> Result<(), String> {
    let dir = utils::get_plugin_dir(&app)?;
    if file_name.is_empty() || file_name.contains(['/', '\\']) || file_name.contains("..") {
        return Err("err_invalid_plugin_name".to_string());
    }

    tokio::task::spawn_blocking(move || {
        let target = dir.join(file_name);
        if target.exists() {
            std::fs::remove_file(&target).map_err(|e| format!("err_delete_file:{}", e))?;
        }
        Ok(())
    })
    .await
    .map_err(|e| format!("err_task:{}", e))?
}

/// 在系统文件管理器中打开插件目录
#[tauri::command]
pub async fn open_plugin_dir(app: AppHandle) -> Result<(), String> {
    let dir = utils::get_plugin_dir(&app)?;
    if !dir.exists() {
        std::fs::create_dir_all(&dir).map_err(|e| format!("err_create_dir:{}", e))?;
    }
    app.opener()
        .open_path(dir.to_string_lossy().to_string(), None::<&str>)
        .map_err(|e| format!("err_open_dir:{}", e))?;
    Ok(())
}

/// 快速检查 zip 是否为 yt-dlp 插件，并提取插件类型（只读中央目录，无需解压文件）
fn inspect_plugin_types(path: &Path) -> Result<Vec<String>, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("err_open_file:{}", e))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|_| "err_plugin_layout".to_string())?;

    let mut has_extractor = false;
    let mut has_postprocessor = false;

    for i in 0..archive.len() {
        if let Ok(entry) = archive.by_index_raw(i) {
            let name = entry.name().replace('\\', "/");
            if name.contains("yt_dlp_plugins/extractor/") && name.ends_with(".py") {
                has_extractor = true;
            }
            if name.contains("yt_dlp_plugins/postprocessor/") && name.ends_with(".py") {
                has_postprocessor = true;
            }
            if has_extractor && has_postprocessor {
                break;
            }
        }
    }

    let mut types = Vec::new();
    if has_extractor {
        types.push("extractor".to_string());
    }
    if has_postprocessor {
        types.push("postprocessor".to_string());
    }

    if types.is_empty() {
        return Err("err_plugin_layout".to_string());
    }

    Ok(types)
}

/// 扫描插件目录
fn scan_plugins(dir: &Path) -> Result<Vec<PluginItem>, String> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }

    let entries = std::fs::read_dir(dir).map_err(|e| format!("err_read_plugin_dir:{}", e))?;
    let mut items = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let is_zip = path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"));

        if !is_zip {
            continue;
        }

        if let Ok(item) = read_plugin_item(&path) {
            items.push(item);
        }
    }

    items.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(items)
}

/// 读取单个插件包的基本元信息
fn read_plugin_item(path: &Path) -> Result<PluginItem, String> {
    let metadata = std::fs::metadata(path).map_err(|e| format!("err_read_plugin:{}", e))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("err_invalid_plugin_name")?
        .to_string();

    let name = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(&file_name)
        .to_string();

    // 快速提取插件类型（若不是合法插件包则标记为空）
    let plugin_types = inspect_plugin_types(path).unwrap_or_default();

    let modified_at = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or_default();

    Ok(PluginItem {
        file_name,
        name,
        size_bytes: metadata.len(),
        modified_at,
        plugin_types,
    })
}
