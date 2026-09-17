import localforage from "localforage";
import type { DownloadTask } from "@/types";
import { normalizeTaskParams } from "@/utils/taskParams";
import { trimTaskLogs } from "@/utils/logs";

/**
 * 老版本下载任务数据源（IndexedDB）的读取与清洗
 *
 * 纯读取/纯清洗函数与副作用（落库/标记）分离，便于单测与后续移除。
 */

/** 老版本 IndexedDB 实例定位 */
const legacyTasksStorage = localforage.createInstance({
  name: "yt-dlp-gui",
  storeName: "downloads",
});

/** 老版本任务列表的存储 key */
export const LEGACY_TASKS_KEY = "download_tasks";

/** 仅统计老版本任务条数（弹窗计数用，不读全量详情） */
export const countLegacyTasks = async (): Promise<number> => {
  try {
    const saved = await legacyTasksStorage.getItem<DownloadTask[]>(LEGACY_TASKS_KEY);
    return Array.isArray(saved) ? saved.length : 0;
  } catch {
    return 0;
  }
};

/** 读取老版本任务全量（原始数据，未清洗） */
export const readLegacyTasks = async (): Promise<DownloadTask[]> => {
  try {
    const saved = await legacyTasksStorage.getItem<DownloadTask[]>(LEGACY_TASKS_KEY);
    return Array.isArray(saved) ? saved : [];
  } catch {
    return [];
  }
};

/**
 * 清洗老版本任务列表（纯函数）
 *
 * 1. 未竟任务（下载中/排队中/准备中/后处理）归一为 error 中断态；
 * 2. 已完成任务清空运行时日志（沿用老版本行为，节省空间）；
 * 3. 其余日志裁剪到环形缓冲上限；
 * 4. 参数补齐默认值；缺 createdAt 回退为当前时间。
 *
 * @param saved 老版本原始任务列表
 * @returns 可直接入库的任务列表
 */
export const sanitizeLegacyTasks = (saved: DownloadTask[]): DownloadTask[] =>
  saved
    .filter((task) => task && typeof task.id === "string" && task.id.trim() !== "")
    .map((task) => {
      const isInterrupted =
        task.status === "downloading" ||
        task.status === "postprocessing" ||
        task.status === "queued" ||
        task.status === "preparing";

      const logs = Array.isArray(task.logs) ? [...task.logs] : [];
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

/**
 * 清空老版本任务数据源
 *
 * 仅在迁移成功并校验通过后由 runner 调用；平时不调用。
 *
 * @returns 清理完成 Promise
 */
export const clearLegacyTasks = async (): Promise<void> => {
  try {
    await legacyTasksStorage.removeItem(LEGACY_TASKS_KEY);
  } catch {
    // 忽略清理异常
  }
};
