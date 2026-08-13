//! 外部工具的远端版本检测。

use std::{cmp::Ordering, time::Duration};

use tauri::AppHandle;

use crate::utils;

use super::{get_deno_status, get_ffmpeg_status, get_ytdlp_status, ToolStatus};

const UPDATE_CHECK_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolUpdateCheck {
    pub update_available: bool,
    pub current_version: String,
    pub latest_version: String,
}

async fn get_status(app: AppHandle, tool: &str) -> Result<ToolStatus, String> {
    match tool {
        "yt-dlp" => get_ytdlp_status(app).await,
        "deno" => get_deno_status(app).await,
        "ffmpeg" => get_ffmpeg_status(app).await,
        _ => Err(format!("err_unknown_tool:{}", tool)),
    }
}

async fn fetch_latest_release_tag(tool: &str) -> Result<String, String> {
    let url = utils::get_tool_latest_release_url(tool)
        .ok_or_else(|| format!("err_unknown_tool:{}", tool))?;
    let client = reqwest::Client::builder()
        .timeout(UPDATE_CHECK_TIMEOUT)
        .user_agent(concat!("yt-dlp-gui/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| format!("err_create_http_client:{}", e))?;
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("err_check_update_failed:{}", e))?
        .error_for_status()
        .map_err(|e| format!("err_check_update_failed:{}", e))?;
    let segments = response
        .url()
        .path_segments()
        .ok_or("err_latest_release_version")?
        .collect::<Vec<_>>();
    segments
        .windows(2)
        .find_map(|parts| (parts[0] == "tag").then(|| parts[1].to_string()))
        .filter(|tag| !tag.is_empty())
        .ok_or_else(|| "err_latest_release_version".to_string())
}

fn version_token(value: &str) -> Option<&str> {
    let start = value.find(|character: char| character.is_ascii_digit())?;
    let candidate = &value[start..];
    let end = candidate
        .find(|character: char| !character.is_ascii_digit() && character != '.')
        .unwrap_or(candidate.len());
    let token = candidate[..end].trim_end_matches('.');
    (!token.is_empty()).then_some(token)
}

fn version_parts(value: &str) -> Result<Vec<u64>, String> {
    version_token(value)
        .ok_or_else(|| format!("err_invalid_tool_version:{}", value))?
        .split('.')
        .map(|part| {
            part.parse::<u64>()
                .map_err(|e| format!("err_invalid_tool_version:{}:{}", value, e))
        })
        .collect()
}

fn compare_versions(left: &str, right: &str) -> Result<Ordering, String> {
    let left = version_parts(left)?;
    let right = version_parts(right)?;
    let length = left.len().max(right.len());
    for index in 0..length {
        let ordering = left
            .get(index)
            .copied()
            .unwrap_or_default()
            .cmp(&right.get(index).copied().unwrap_or_default());
        if ordering != Ordering::Equal {
            return Ok(ordering);
        }
    }
    Ok(Ordering::Equal)
}

#[tauri::command]
pub async fn check_tool_update(app: AppHandle, tool: String) -> Result<ToolUpdateCheck, String> {
    let status = get_status(app, &tool).await?;
    if !status.installed {
        return Err(format!("err_tool_not_installed:{}", tool));
    }
    if !status.can_update {
        return Err(format!("err_tool_update_not_supported:{}", tool));
    }

    let latest_tag = fetch_latest_release_tag(&tool).await?;
    let current_version = version_token(&status.version)
        .ok_or_else(|| format!("err_invalid_tool_version:{}", status.version))?
        .to_string();
    let latest_version = version_token(&latest_tag)
        .ok_or_else(|| format!("err_invalid_tool_version:{}", latest_tag))?
        .to_string();
    let update_available =
        compare_versions(&latest_version, &current_version)? == Ordering::Greater;

    Ok(ToolUpdateCheck {
        update_available,
        current_version,
        latest_version,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_versions_from_all_supported_formats() {
        assert_eq!(version_token("2026.07.04"), Some("2026.07.04"));
        assert_eq!(version_token("v2.9.5"), Some("2.9.5"));
        assert_eq!(version_token("b6.1.1"), Some("6.1.1"));
        assert_eq!(
            version_token("2.9.5 (stable, release, x86_64-pc-windows-msvc)"),
            Some("2.9.5")
        );
        assert_eq!(
            version_token("8.0.1-essentials_build-www.gyan.dev"),
            Some("8.0.1")
        );
    }

    #[test]
    fn compares_numeric_version_segments() {
        assert_eq!(
            compare_versions("2026.07.04", "2026.03.17"),
            Ok(Ordering::Greater)
        );
        assert_eq!(compare_versions("v2.9.5", "2.9.5"), Ok(Ordering::Equal));
        assert_eq!(compare_versions("b6.1.1", "6.0"), Ok(Ordering::Greater));
        assert_eq!(compare_versions("6.1.1", "8.0.1"), Ok(Ordering::Less));
    }
}
