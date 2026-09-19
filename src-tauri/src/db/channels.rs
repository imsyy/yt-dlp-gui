//! 主播/频道与视频条目的 SQLite 数据持久化与查询。

use super::DatabaseState;
use rusqlite::{params, OptionalExtension};

/// 频道记录实体
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelRecord {
    pub id: String,
    pub url: String,
    pub title: String,
    pub uploader: String,
    pub uploader_id: String,
    pub avatar: String,
    pub banner: String,
    pub description: String,
    pub platform: String,
    pub subscriber_count: Option<i64>,
    pub video_count: i64,
    pub last_synced_at: Option<i64>,
    pub sync_status: String,
    pub sync_error: Option<String>,
    pub created_at: i64,
}

/// 频道下视频记录实体
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelVideoRecord {
    pub id: String,
    pub channel_id: String,
    pub video_id: String,
    pub url: String,
    pub title: String,
    pub thumbnail: String,
    pub duration: Option<f64>,
    pub view_count: Option<i64>,
    pub published_at: Option<i64>,
    /// 发布日期精度：'approx'（flat-playlist 近似值）| 'exact'（逐视频详情校准值）
    pub published_accuracy: String,
    pub content_type: String,
    pub created_at: i64,
}

/// 视频列表分页查询参数
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelVideosQuery {
    pub channel_id: String,
    pub content_type: Option<String>, // "all" | "video" | "stream" | "short"
    pub query: Option<String>,
    pub sort_by: Option<String>, // "published_at" | "view_count" | "duration"
    pub sort_order: Option<String>, // "asc" | "desc"
    pub page: u32,
    pub page_size: u32,
}

/// 视频分页结果
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelVideosPage {
    pub items: Vec<ChannelVideoRecord>,
    pub total: i64,
    pub page: u32,
    pub page_size: u32,
}

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}

/// 添加或更新频道记录
pub fn upsert_channel(db: &DatabaseState, channel: &ChannelRecord) -> Result<(), String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        r#"
        INSERT INTO channels (
            id, url, title, uploader, uploader_id, avatar, banner,
            description, platform, subscriber_count, video_count,
            last_synced_at, sync_status, sync_error, created_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
        ON CONFLICT(id) DO UPDATE SET
            url = excluded.url,
            title = excluded.title,
            uploader = excluded.uploader,
            uploader_id = excluded.uploader_id,
            avatar = CASE WHEN excluded.avatar != '' THEN excluded.avatar ELSE channels.avatar END,
            banner = CASE WHEN excluded.banner != '' THEN excluded.banner ELSE channels.banner END,
            description = excluded.description,
            platform = excluded.platform,
            subscriber_count = COALESCE(excluded.subscriber_count, channels.subscriber_count)
        "#,
        params![
            channel.id,
            channel.url,
            channel.title,
            channel.uploader,
            channel.uploader_id,
            channel.avatar,
            channel.banner,
            channel.description,
            channel.platform,
            channel.subscriber_count,
            channel.video_count,
            channel.last_synced_at,
            channel.sync_status,
            channel.sync_error,
            channel.created_at,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 根据 ID 获取频道
pub fn get_channel(db: &DatabaseState, channel_id: &str) -> Result<Option<ChannelRecord>, String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    conn.query_row(
        r#"
        SELECT
            id, url, title, uploader, uploader_id, avatar, banner,
            description, platform, subscriber_count, video_count,
            last_synced_at, sync_status, sync_error, created_at
        FROM channels WHERE id = ?1
        "#,
        params![channel_id],
        |row| {
            Ok(ChannelRecord {
                id: row.get(0)?,
                url: row.get(1)?,
                title: row.get(2)?,
                uploader: row.get(3)?,
                uploader_id: row.get(4)?,
                avatar: row.get(5)?,
                banner: row.get(6)?,
                description: row.get(7)?,
                platform: row.get(8)?,
                subscriber_count: row.get(9)?,
                video_count: row.get(10)?,
                last_synced_at: row.get(11)?,
                sync_status: row.get(12)?,
                sync_error: row.get(13)?,
                created_at: row.get(14)?,
            })
        },
    )
    .optional()
    .map_err(|e| e.to_string())
}

/// 根据 URL 查询频道
#[allow(dead_code)]
pub fn get_channel_by_url(db: &DatabaseState, url: &str) -> Result<Option<ChannelRecord>, String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    conn.query_row(
        r#"
        SELECT
            id, url, title, uploader, uploader_id, avatar, banner,
            description, platform, subscriber_count, video_count,
            last_synced_at, sync_status, sync_error, created_at
        FROM channels WHERE url = ?1
        "#,
        params![url],
        |row| {
            Ok(ChannelRecord {
                id: row.get(0)?,
                url: row.get(1)?,
                title: row.get(2)?,
                uploader: row.get(3)?,
                uploader_id: row.get(4)?,
                avatar: row.get(5)?,
                banner: row.get(6)?,
                description: row.get(7)?,
                platform: row.get(8)?,
                subscriber_count: row.get(9)?,
                video_count: row.get(10)?,
                last_synced_at: row.get(11)?,
                sync_status: row.get(12)?,
                sync_error: row.get(13)?,
                created_at: row.get(14)?,
            })
        },
    )
    .optional()
    .map_err(|e| e.to_string())
}

/// 获取所有频道列表
pub fn list_channels(db: &DatabaseState) -> Result<Vec<ChannelRecord>, String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            r#"
            SELECT
                id, url, title, uploader, uploader_id, avatar, banner,
                description, platform, subscriber_count, video_count,
                last_synced_at, sync_status, sync_error, created_at
            FROM channels
            ORDER BY COALESCE(last_synced_at, created_at) DESC
            "#,
        )
        .map_err(|e| e.to_string())?;

    let channels = stmt
        .query_map([], |row| {
            Ok(ChannelRecord {
                id: row.get(0)?,
                url: row.get(1)?,
                title: row.get(2)?,
                uploader: row.get(3)?,
                uploader_id: row.get(4)?,
                avatar: row.get(5)?,
                banner: row.get(6)?,
                description: row.get(7)?,
                platform: row.get(8)?,
                subscriber_count: row.get(9)?,
                video_count: row.get(10)?,
                last_synced_at: row.get(11)?,
                sync_status: row.get(12)?,
                sync_error: row.get(13)?,
                created_at: row.get(14)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .collect();

    Ok(channels)
}

/// 删除频道及其全部归档视频
pub fn delete_channel(db: &DatabaseState, channel_id: &str) -> Result<(), String> {
    let mut conn = db.conn.lock().map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute(
        "DELETE FROM channel_videos WHERE channel_id = ?1",
        params![channel_id],
    )
    .map_err(|e| e.to_string())?;
    tx.execute(
        "DELETE FROM channels WHERE id = ?1",
        params![channel_id],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

/// 更新频道同步状态
pub fn update_channel_sync_status(
    db: &DatabaseState,
    channel_id: &str,
    status: &str,
    error: Option<&str>,
) -> Result<(), String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    let now = now_millis();
    if status == "completed" {
        let video_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM channel_videos WHERE channel_id = ?1",
                params![channel_id],
                |row| row.get(0),
            )
            .unwrap_or(0);
        conn.execute(
            r#"
            UPDATE channels SET
                sync_status = 'idle',
                sync_error = NULL,
                video_count = ?2,
                last_synced_at = ?3
            WHERE id = ?1
            "#,
            params![channel_id, video_count, now],
        )
        .map_err(|e| e.to_string())?;
    } else if status == "error" {
        conn.execute(
            r#"
            UPDATE channels SET
                sync_status = 'error',
                sync_error = ?2
            WHERE id = ?1
            "#,
            params![channel_id, error],
        )
        .map_err(|e| e.to_string())?;
    } else {
        conn.execute(
            r#"
            UPDATE channels SET
                sync_status = ?2,
                sync_error = NULL
            WHERE id = ?1
            "#,
            params![channel_id, status],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// 获取频道已存在的所有 video_id 集合（用于增量同步命中比对）
pub fn get_channel_existing_video_ids(
    db: &DatabaseState,
    channel_id: &str,
) -> Result<std::collections::HashSet<String>, String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT video_id FROM channel_videos WHERE channel_id = ?1")
        .map_err(|e| e.to_string())?;
    let ids = stmt
        .query_map(params![channel_id], |row| row.get::<_, String>(0))
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .collect();
    Ok(ids)
}

/// 批量插入/更新视频条目
pub fn upsert_channel_videos_batch(
    db: &DatabaseState,
    videos: &[ChannelVideoRecord],
) -> Result<usize, String> {
    if videos.is_empty() {
        return Ok(0);
    }
    let mut conn = db.conn.lock().map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let mut affected = 0;
    {
        let mut stmt = tx
            .prepare(
                r#"
                INSERT INTO channel_videos (
                    id, channel_id, video_id, url, title, thumbnail,
                    duration, view_count, published_at, published_accuracy, content_type, created_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
                ON CONFLICT(id) DO UPDATE SET
                    title = excluded.title,
                    thumbnail = CASE WHEN excluded.thumbnail != '' THEN excluded.thumbnail ELSE channel_videos.thumbnail END,
                    duration = COALESCE(excluded.duration, channel_videos.duration),
                    view_count = COALESCE(excluded.view_count, channel_videos.view_count),
                    -- 已校准的精确日期不允许被后续 flat 同步的近似值覆盖
                    published_at = CASE
                        WHEN channel_videos.published_accuracy = 'exact' AND excluded.published_accuracy = 'approx'
                        THEN channel_videos.published_at
                        ELSE COALESCE(excluded.published_at, channel_videos.published_at)
                    END,
                    published_accuracy = CASE
                        WHEN channel_videos.published_accuracy = 'exact' AND excluded.published_accuracy = 'approx'
                        THEN channel_videos.published_accuracy
                        ELSE excluded.published_accuracy
                    END,
                    content_type = excluded.content_type
                "#,
            )
            .map_err(|e| e.to_string())?;

        for v in videos {
            stmt.execute(params![
                v.id,
                v.channel_id,
                v.video_id,
                v.url,
                v.title,
                v.thumbnail,
                v.duration,
                v.view_count,
                v.published_at,
                v.published_accuracy,
                v.content_type,
                v.created_at,
            ])
            .map_err(|e| e.to_string())?;
            affected += 1;
        }
    }
    tx.commit().map_err(|e| e.to_string())?;

    if let Some(first) = videos.first() {
        let channel_id = &first.channel_id;
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM channel_videos WHERE channel_id = ?1",
                params![channel_id],
                |row| row.get(0),
            )
            .unwrap_or(0);
        let _ = conn.execute(
            "UPDATE channels SET video_count = ?2 WHERE id = ?1",
            params![channel_id, count],
        );
    }

    Ok(affected)
}

/// 日期校准任务的目标视频（行主键 + 视频 ID + 详情页 URL）
pub struct EnrichTarget {
    pub id: String,
    pub video_id: String,
    pub url: String,
}

/// 查询需要校准发布日期的视频：指定 video_id 列表则按列表取，
/// 否则取该频道下精度非 'exact'（含日期为空）的全部记录。
/// 两个分支都只返回未精确的行，已精确的行永远不会被重复逐条抓取。
pub fn get_enrich_targets(
    db: &DatabaseState,
    channel_id: &str,
    video_ids: Option<&[String]>,
) -> Result<Vec<EnrichTarget>, String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    let mut targets = Vec::new();
    if let Some(ids) = video_ids {
        let ids: Vec<&str> = ids
            .iter()
            .map(String::as_str)
            .filter(|s| !s.is_empty())
            .collect();
        if ids.is_empty() {
            return Ok(targets);
        }
        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT id, video_id, url FROM channel_videos \
             WHERE channel_id = ?1 AND video_id IN ({}) \
             AND (published_at IS NULL OR published_accuracy != 'exact')",
            placeholders
        );
        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> =
            vec![Box::new(channel_id.to_string())];
        for id in &ids {
            params_vec.push(Box::new(id.to_string()));
        }
        let rusqlite_params: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|b| b.as_ref()).collect();
        let rows = stmt
            .query_map(rusqlite_params.as_slice(), |row| {
                Ok(EnrichTarget {
                    id: row.get(0)?,
                    video_id: row.get(1)?,
                    url: row.get(2)?,
                })
            })
            .map_err(|e| e.to_string())?;
        for row in rows.filter_map(Result::ok) {
            targets.push(row);
        }
    } else {
        let mut stmt = conn
            .prepare(
                "SELECT id, video_id, url FROM channel_videos \
                 WHERE channel_id = ?1 AND (published_at IS NULL OR published_accuracy != 'exact')",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![channel_id], |row| {
                Ok(EnrichTarget {
                    id: row.get(0)?,
                    video_id: row.get(1)?,
                    url: row.get(2)?,
                })
            })
            .map_err(|e| e.to_string())?;
        for row in rows.filter_map(Result::ok) {
            targets.push(row);
        }
    }
    Ok(targets)
}

/// 日期校准回填的单行更新：精确日期 + 详情页顺带拿到的其他字段。
/// 标题/封面用空字符串表示“未取到，不覆盖”；时长/播放量用 None 表示不覆盖。
#[derive(Debug)]
pub struct EnrichUpdate {
    pub id: String,
    pub published_at: i64,
    pub title: String,
    pub thumbnail: String,
    pub duration: Option<f64>,
    pub view_count: Option<i64>,
}

/// 批量写入校准结果：日期强制覆盖并标记 'exact'，其他字段只在取到有效值时更新。
/// 返回实际更新的行数。
pub fn update_video_enriched_fields(
    db: &DatabaseState,
    items: &[EnrichUpdate],
) -> Result<usize, String> {
    if items.is_empty() {
        return Ok(0);
    }
    let mut conn = db.conn.lock().map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let mut updated = 0;
    {
        let mut stmt = tx
            .prepare(
                "UPDATE channel_videos SET
                    published_at = ?2, published_accuracy = 'exact',
                    title = CASE WHEN ?3 != '' THEN ?3 ELSE title END,
                    thumbnail = CASE WHEN ?4 != '' THEN ?4 ELSE thumbnail END,
                    duration = COALESCE(?5, duration),
                    view_count = COALESCE(?6, view_count)
                 WHERE id = ?1",
            )
            .map_err(|e| e.to_string())?;
        for item in items {
            updated += stmt
                .execute(params![
                    item.id,
                    item.published_at,
                    item.title,
                    item.thumbnail,
                    item.duration,
                    item.view_count,
                ])
                .map_err(|e| e.to_string())?;
        }
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(updated)
}

/// 分页查询视频列表
pub fn get_channel_videos_page(
    db: &DatabaseState,
    query: &ChannelVideosQuery,
) -> Result<ChannelVideosPage, String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;

    let mut where_clauses = vec!["channel_id = ?1".to_string()];
    let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(query.channel_id.clone())];

    if let Some(ref ct) = query.content_type {
        if ct != "all" && !ct.is_empty() {
            params_vec.push(Box::new(ct.clone()));
            where_clauses.push(format!("content_type = ?{}", params_vec.len()));
        }
    }

    if let Some(ref q) = query.query {
        let trimmed = q.trim();
        if !trimmed.is_empty() {
            params_vec.push(Box::new(format!("%{}%", trimmed)));
            where_clauses.push(format!("title LIKE ?{}", params_vec.len()));
        }
    }

    let where_str = where_clauses.join(" AND ");

    let count_sql = format!("SELECT COUNT(*) FROM channel_videos WHERE {}", where_str);
    let mut count_stmt = conn.prepare(&count_sql).map_err(|e| e.to_string())?;
    let rusqlite_params: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|b| b.as_ref()).collect();
    let total: i64 = count_stmt
        .query_row(rusqlite_params.as_slice(), |row| row.get(0))
        .map_err(|e| e.to_string())?;

    let sort_col = match query.sort_by.as_deref() {
        Some("view_count") => "view_count",
        Some("duration") => "duration",
        _ => "COALESCE(published_at, created_at)",
    };
    let sort_dir = match query.sort_order.as_deref() {
        Some("asc") => "ASC",
        _ => "DESC",
    };

    let page = query.page.max(1);
    // 上限与前端 channel_get_videos 的单次抓取批量保持一致，前端按此批量翻页取全量
    let page_size = query.page_size.clamp(1, 1000);
    let offset = (page - 1) * page_size;

    params_vec.push(Box::new(page_size));
    let limit_param_idx = params_vec.len();
    params_vec.push(Box::new(offset));
    let offset_param_idx = params_vec.len();

    let query_sql = format!(
        r#"
        SELECT
            id, channel_id, video_id, url, title, thumbnail,
            duration, view_count, published_at, published_accuracy, content_type, created_at
        FROM channel_videos
        WHERE {}
        ORDER BY {} {}
        LIMIT ?{} OFFSET ?{}
        "#,
        where_str, sort_col, sort_dir, limit_param_idx, offset_param_idx
    );

    let mut query_stmt = conn.prepare(&query_sql).map_err(|e| e.to_string())?;
    let rusqlite_params: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|b| b.as_ref()).collect();

    let items = query_stmt
        .query_map(rusqlite_params.as_slice(), |row| {
            Ok(ChannelVideoRecord {
                id: row.get(0)?,
                channel_id: row.get(1)?,
                video_id: row.get(2)?,
                url: row.get(3)?,
                title: row.get(4)?,
                thumbnail: row.get(5)?,
                duration: row.get(6)?,
                view_count: row.get(7)?,
                published_at: row.get(8)?,
                published_accuracy: row.get(9)?,
                content_type: row.get(10)?,
                created_at: row.get(11)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .collect();

    Ok(ChannelVideosPage {
        items,
        total,
        page,
        page_size,
    })
}
