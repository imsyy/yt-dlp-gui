import localforage from "localforage";
import { invoke } from "@tauri-apps/api/core";
import type { DownloadTask, DownloadTaskParams } from "@/types";
import type { HistoryItem } from "@/stores/history";
import { trimTaskLogs } from "@/utils/logs";

/** 老版本 IndexedDB 实例 */
const legacyTasksStorage = localforage.createInstance({
  name: "yt-dlp-gui",
  storeName: "downloads",
});

const TASKS_STORAGE_KEY = "download_tasks";
const TASKS_MIGRATED_FLAG = "yt_dlp_gui_tasks_migrated_v1";
const HISTORY_MIGRATED_FLAG = "yt_dlp_gui_history_migrated_v1";

/**
 * 补齐下载任务参数默认值，防止字段缺失导致后端反序列化报错
 *
 * @param params 原始或部分下载参数
 * @returns 补齐默认值后的完整参数对象
 */
export const normalizeTaskParams = (params?: Partial<DownloadTaskParams>): DownloadTaskParams => ({
  url: params?.url || "",
  downloadDir: params?.downloadDir || "",
  downloadMode: params?.downloadMode || "default",
  videoFormat: params?.videoFormat ?? null,
  audioFormat: params?.audioFormat ?? null,
  cookieFile: params?.cookieFile ?? null,
  cookieBrowser: params?.cookieBrowser ?? null,
  proxy: params?.proxy ?? null,
  outputTemplate: params?.outputTemplate ?? null,
  concurrentFragments: params?.concurrentFragments ?? null,
  noOverwrites: Boolean(params?.noOverwrites),
  embedSubs: Boolean(params?.embedSubs),
  embedThumbnail: Boolean(params?.embedThumbnail),
  writeThumbnail: Boolean(params?.writeThumbnail),
  writeDescription: Boolean(params?.writeDescription),
  embedMetadata: Boolean(params?.embedMetadata),
  embedChapters: Boolean(params?.embedChapters),
  sponsorblockRemove: Boolean(params?.sponsorblockRemove),
  extractAudio: Boolean(params?.extractAudio),
  audioConvertFormat: params?.audioConvertFormat ?? null,
  noMerge: Boolean(params?.noMerge),
  recodeFormat: params?.recodeFormat ?? null,
  remuxFormat: params?.remuxFormat ?? null,
  limitRate: params?.limitRate ?? null,
  ffmpegArgs: params?.ffmpegArgs ?? null,
  customArgs: params?.customArgs ?? null,
  subtitles: Array.isArray(params?.subtitles) ? params.subtitles : [],
  startTime: params?.startTime ?? null,
  endTime: params?.endTime ?? null,
  noPlaylist: Boolean(params?.noPlaylist),
  playlistItems: params?.playlistItems ?? null,
  liveFromStart: Boolean(params?.liveFromStart),
});

/**
 * 将老版本保存在 IndexedDB 中的任务列表平滑迁移至 SQLite
 *
 * @returns 迁移完成 Promise
 */
export const migrateLegacyTasks = async (): Promise<void> => {
  if (typeof window === "undefined" || !window.localStorage) {
    return;
  }

  if (localStorage.getItem(TASKS_MIGRATED_FLAG) === "true") {
    return;
  }

  try {
    const saved = await legacyTasksStorage.getItem<DownloadTask[]>(TASKS_STORAGE_KEY);
    if (saved && Array.isArray(saved) && saved.length > 0) {
      const sanitizedTasks: DownloadTask[] = saved.map((task) => {
        const isInterrupted =
          task.status === "downloading" ||
          task.status === "postprocessing" ||
          task.status === "queued" ||
          task.status === "preparing";

        const logs = Array.isArray(task.logs) ? [...task.logs] : [];
        // 已完成任务不需要运行时日志（沿用老版本 IndexedDB 行为，节省空间）；
        // 其余任务的日志裁剪到环形缓冲上限后再入库
        if (task.status === "completed") {
          logs.length = 0;
        } else {
          trimTaskLogs(logs);
        }

        return {
          ...task,
          status: isInterrupted ? "error" : task.status,
          error: isInterrupted ? task.error || "应用已升级重启，历史未竟任务已中断" : task.error,
          speed: isInterrupted ? "" : task.speed,
          eta: isInterrupted ? "" : task.eta,
          logs,
          createdAt: task.createdAt || Date.now(),
          params: normalizeTaskParams(task.params),
        };
      });

      await invoke("db_upsert_tasks_batch", { tasks: sanitizedTasks });
    }

    localStorage.setItem(TASKS_MIGRATED_FLAG, "true");
  } catch (error) {
    console.warn("老版本下载任务迁移至 SQLite 失败，将在下次启动时重试:", error);
  }
};

/**
 * 将老版本保存在 localStorage 中的解析历史平滑迁移至 SQLite
 *
 * @returns 迁移得到的历史列表（如果已迁移或无数据则返回 null）
 */
export const migrateLegacyHistory = async (): Promise<HistoryItem[] | null> => {
  if (typeof window === "undefined" || !window.localStorage) {
    return null;
  }

  if (localStorage.getItem(HISTORY_MIGRATED_FLAG) === "true") {
    return null;
  }

  const oldRaw = localStorage.getItem("history");
  if (!oldRaw) {
    localStorage.setItem(HISTORY_MIGRATED_FLAG, "true");
    return null;
  }

  try {
    const parsed = JSON.parse(oldRaw);
    let rawList: Array<{ url?: string; title?: string; time?: number } | string> = [];

    if (Array.isArray(parsed?.items)) {
      rawList = parsed.items;
    } else if (Array.isArray(parsed)) {
      rawList = parsed;
    } else if (Array.isArray(parsed?.urls)) {
      rawList = parsed.urls;
    }

    const validHistory: HistoryItem[] = rawList
      .map((entry) => {
        if (typeof entry === "string") {
          const trimmed = entry.trim();
          return trimmed ? { url: trimmed, title: trimmed, time: Date.now() } : null;
        }
        if (entry && typeof entry.url === "string") {
          const trimmed = entry.url.trim();
          return trimmed
            ? {
                url: trimmed,
                title: entry.title?.trim() || trimmed,
                time: entry.time || Date.now(),
              }
            : null;
        }
        return null;
      })
      .filter((item): item is HistoryItem => item !== null);

    if (validHistory.length > 0) {
      await invoke("db_add_history_batch", { items: validHistory });
    }

    localStorage.removeItem("history");
    localStorage.setItem(HISTORY_MIGRATED_FLAG, "true");
    return validHistory;
  } catch (error) {
    console.warn("老版本解析历史迁移至 SQLite 失败:", error);
    return null;
  }
};
