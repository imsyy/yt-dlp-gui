//! 下载结果文件操作命令。

/// yt-dlp 写盘过程中的临时后缀（未完成的下载与元数据文件）
const TEMP_SUFFIXES: [&str; 2] = [".part", ".ytdl"];

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

/// 判断同一目录下的某个文件名是否属于给定输出文件。
///
/// 候选名去掉可选的临时后缀后，必须正好等于 `output_name`，或等于
/// 「输出主干 + `.fNNN.<ext>`」这类 yt-dlp 分片中间产物命名。
///
/// 关键约束：**绝不能退化成「包含标题」**。用标题做子串匹配会让标题为前缀的
/// 另一个任务被误伤——清理任务 `My Video` 时会删掉另一个任务正在下载的
/// `My Video 2.f137.mp4.part`，直接破坏对方的下载。
fn belongs_to_output(candidate: &str, output_name: &str) -> bool {
    let core = TEMP_SUFFIXES
        .iter()
        .find_map(|suffix| candidate.strip_suffix(suffix))
        .unwrap_or(candidate);
    if core == output_name {
        return true;
    }

    let stem = output_name
        .rsplit_once('.')
        .map_or(output_name, |(stem, _)| stem);
    let Some(rest) = core.strip_prefix(stem) else {
        return false;
    };
    // 主干之后必须紧跟一个点，且分片产物的第一段必须为分片格式（如 .f137.mp4 / .f137）
    rest.starts_with('.') && {
        let mut parts = rest[1..].split('.');
        parts
            .next()
            .is_some_and(|f| f.starts_with('f') && f.len() > 1 && f[1..].chars().all(|c| c.is_ascii_digit()))
    }
}

/// 清理指定任务产生的残留文件（`.part`、`.ytdl` 与分片中间产物）。
///
/// 只依据调用方给出的**精确输出路径**推导，因此不会再出现「按标题模糊匹配」
/// 带来的跨任务误删。`known_paths` 为空时不做任何事——没有已知输出就没有
/// 可以安全归属到该任务的残留。
#[tauri::command]
pub fn clean_task_residual_files(known_paths: Vec<String>) -> Result<Vec<String>, String> {
    let mut deleted = Vec::new();
    let mut visited = std::collections::HashSet::new();

    for raw in &known_paths {
        let output = raw.trim();
        if output.is_empty() {
            continue;
        }
        let output_path = std::path::Path::new(output);

        // 输出文件本身及其临时后缀
        remove_if_file(output_path, &mut visited, &mut deleted);
        for suffix in TEMP_SUFFIXES {
            remove_if_file(
                &std::path::PathBuf::from(format!("{output}{suffix}")),
                &mut visited,
                &mut deleted,
            );
        }

        // 同目录下的分片中间产物
        let (Some(dir), Some(output_name)) = (
            output_path.parent(),
            output_path.file_name().and_then(|name| name.to_str()),
        ) else {
            continue;
        };
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if !file_type.is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if belongs_to_output(&name, output_name) {
                remove_if_file(&entry.path(), &mut visited, &mut deleted);
            }
        }
    }

    Ok(deleted)
}

/// 删除文件并记录；去重避免同一路径被处理两次
fn remove_if_file(
    path: &std::path::Path,
    visited: &mut std::collections::HashSet<String>,
    deleted: &mut Vec<String>,
) {
    if !path.is_file() {
        return;
    }
    let key = path.to_string_lossy().to_string();
    if visited.insert(key.clone()) && std::fs::remove_file(path).is_ok() {
        deleted.push(key);
    }
}

#[cfg(test)]
mod tests {
    use super::belongs_to_output;

    #[test]
    fn matches_the_output_file_and_its_temp_suffixes() {
        assert!(belongs_to_output("video.mp4", "video.mp4"));
        assert!(belongs_to_output("video.mp4.part", "video.mp4"));
        assert!(belongs_to_output("video.mp4.ytdl", "video.mp4"));
    }

    #[test]
    fn matches_yt_dlp_fragment_intermediates() {
        assert!(belongs_to_output("video.f137.mp4", "video.mp4"));
        assert!(belongs_to_output("video.f137.mp4.part", "video.mp4"));
        assert!(belongs_to_output("video.f140.m4a.part", "video.mp4"));
        // 无扩展名的输出模板同样适用
        assert!(belongs_to_output("video.f137", "video"));
    }

    /// 回归测试：标题为前缀的其它任务不得被误伤
    #[test]
    fn does_not_match_a_longer_name_sharing_the_same_prefix() {
        assert!(!belongs_to_output("My Video 2.f137.mp4.part", "My Video.mp4"));
        assert!(!belongs_to_output("My Video 2.mp4.part", "My Video.mp4"));
        assert!(!belongs_to_output("video-extra.mp4.part", "video.mp4"));
        // 主干之后不是合法后缀
        assert!(!belongs_to_output("video.mkv", "video.mp4"));
        assert!(!belongs_to_output("video.", "video.mp4"));
        assert!(!belongs_to_output("videof137.mp4", "video.mp4"));
    }
}
