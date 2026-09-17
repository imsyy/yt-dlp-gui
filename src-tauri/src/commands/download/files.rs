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

/// 清理指定任务在目标目录中产生的残留文件（如 .part、.ytdl、临时分片等）
#[tauri::command]
pub fn clean_task_residual_files(
    download_dir: String,
    title: String,
    known_paths: Vec<String>,
) -> Result<Vec<String>, String> {
    let mut deleted = Vec::new();
    let mut visited_paths = std::collections::HashSet::new();

    // 1. 处理前端已知的相关文件路径（outputFile 或从日志中提取的目标文件）
    for path_str in known_paths {
        let trimmed = path_str.trim();
        if trimmed.is_empty() {
            continue;
        }
        let p = std::path::Path::new(trimmed);

        // 尝试删除文件本身
        if p.is_file() {
            let key = p.to_string_lossy().to_string();
            if visited_paths.insert(key.clone()) && std::fs::remove_file(p).is_ok() {
                deleted.push(key);
            }
        }

        // 尝试删除 .part 文件
        let part_path = std::path::PathBuf::from(format!("{}.part", trimmed));
        if part_path.is_file() {
            let key = part_path.to_string_lossy().to_string();
            if visited_paths.insert(key.clone()) && std::fs::remove_file(&part_path).is_ok() {
                deleted.push(key);
            }
        }

        // 尝试删除 .ytdl 文件
        let ytdl_path = std::path::PathBuf::from(format!("{}.ytdl", trimmed));
        if ytdl_path.is_file() {
            let key = ytdl_path.to_string_lossy().to_string();
            if visited_paths.insert(key.clone()) && std::fs::remove_file(&ytdl_path).is_ok() {
                deleted.push(key);
            }
        }
    }

    // 2. 在 download_dir 目录下，根据 title 匹配残留的临时分片文件
    let dir = std::path::Path::new(&download_dir);
    if dir.is_dir() {
        let safe_title: String = title
            .chars()
            .filter(|c| !['\\', '/', ':', '*', '?', '"', '<', '>', '|'].contains(c))
            .collect();
        let safe_title = safe_title.trim();

        if safe_title.chars().count() >= 3 {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    if let Ok(file_type) = entry.file_type() {
                        if !file_type.is_file() {
                            continue;
                        }
                        let path = entry.path();
                        let file_name = entry.file_name().to_string_lossy().to_string();

                        let is_temp_ext = file_name.ends_with(".part")
                            || file_name.ends_with(".ytdl")
                            || file_name.contains(".temp.");

                        if is_temp_ext && file_name.contains(safe_title) {
                            let key = path.to_string_lossy().to_string();
                            if visited_paths.insert(key.clone()) && std::fs::remove_file(&path).is_ok() {
                                deleted.push(key);
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(deleted)
}

