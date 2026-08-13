//! 下载结果文件操作命令。

/// 批量检查文件是否存在
#[tauri::command]
pub fn check_files_exist(paths: Vec<String>) -> Vec<bool> {
    paths
        .iter()
        .map(|p| std::path::Path::new(p).exists())
        .collect()
}

/// 删除指定文件
#[tauri::command]
pub fn delete_file(path: String) -> Result<(), String> {
    let p = std::path::Path::new(&path);
    if p.exists() {
        std::fs::remove_file(p).map_err(|e| format!("err_delete_file:{}", e))?;
    }
    Ok(())
}
