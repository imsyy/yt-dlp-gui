import { invoke } from "@tauri-apps/api/core";
import type { DownloadTask } from "@/types";
import type { HistoryItem } from "@/stores/history";
import {
  clearLegacyHistory,
  countLegacyHistory,
  readLegacyHistory,
} from "./legacyHistory";
import {
  clearLegacyTasks,
  countLegacyTasks,
  readLegacyTasks,
  sanitizeLegacyTasks,
} from "./legacyTasks";
import {
  needsMigrationPrompt,
  type LegacyDataSummary,
  type MigrationLedger,
  type MigrationProgress,
} from "./types";

/**
 * 老版本数据迁移编排器
 *
 * 职责：检测 → 计数 →（用户确认后）迁移 → 校验 → 清理旧源 → 标记。
 * 不负责弹窗 UI（见 components/MigrationModal.vue）与 store 重载（调用方负责）。
 */

/** 迁移总账的存储 key */
export const MIGRATION_LEDGER_KEY = "yt_dlp_gui_legacy_migration";

const hasStorage = (): boolean => typeof window !== "undefined" && !!window.localStorage;

const defaultLedger = (): MigrationLedger => ({ tasks: "pending", history: "pending" });

/**
 * 读取迁移总账，不存在则初始化
 *
 * @returns 当前总账
 */
export const readLedger = (): MigrationLedger => {
  const ledger = defaultLedger();
  if (!hasStorage()) return ledger;
  try {
    const raw = localStorage.getItem(MIGRATION_LEDGER_KEY);
    if (raw) {
      const parsed = JSON.parse(raw);
      if (parsed?.tasks === "done") ledger.tasks = "done";
      if (parsed?.history === "done") ledger.history = "done";
    }
  } catch {
    // 解析失败按未迁移处理
  }
  return ledger;
};

/**
 * 持久化迁移总账
 *
 * @param ledger 待保存的总账
 */
export const writeLedger = (ledger: MigrationLedger): void => {
  if (!hasStorage()) return;
  try {
    localStorage.setItem(MIGRATION_LEDGER_KEY, JSON.stringify(ledger));
  } catch {
    // 配额异常等直接忽略，下次启动重新检测
  }
};

export interface DetectionResult {
  summary: LegacyDataSummary;
  ledger: MigrationLedger;
  needsPrompt: boolean;
}

/**
 * 检测老版本数据（只计数，不迁移）
 *
 * 无数据的域会直接标 done，避免每次启动重复检测。
 *
 * @returns 计数汇总、总账与是否需要弹窗
 */
export const detectLegacyData = async (): Promise<DetectionResult> => {
  const ledger = readLedger();
  const summary: LegacyDataSummary = { tasks: 0, history: 0 };

  if (ledger.tasks === "pending") {
    summary.tasks = await countLegacyTasks();
    if (summary.tasks === 0) {
      ledger.tasks = "done";
    }
  }
  if (ledger.history === "pending") {
    summary.history = countLegacyHistory();
    if (summary.history === 0) {
      ledger.history = "done";
    }
  }
  writeLedger(ledger);

  return { summary, ledger, needsPrompt: needsMigrationPrompt(summary) };
};

/**
 * 校验任务 id 是否都已落库
 *
 * @param ids 待校验的任务 id 列表
 */
const verifyTasksLanded = async (ids: string[]): Promise<boolean> => {
  if (ids.length === 0) return true;
  const landed = await invoke<DownloadTask[]>("db_get_tasks");
  const landedIds = new Set(landed.map((task) => task.id));
  return ids.every((id) => landedIds.has(id));
};

/**
 * 校验历史 url 是否都已落库
 *
 * @param urls 待校验的历史 url 列表
 */
const verifyHistoryLanded = async (urls: string[]): Promise<boolean> => {
  if (urls.length === 0) return true;
  const landed = await invoke<HistoryItem[]>("db_get_history");
  const landedUrls = new Set(landed.map((item) => item.url));
  return urls.every((url) => landedUrls.has(url));
};

/**
 * 执行待迁移域的数据搬运（含校验与旧数据源清理）
 *
 * 按"读 → 洗 → 批量入库 → 回读校验 → 清理旧源 → 标记"串行，任一步失败即抛错，
 * 成功才标记 done，失败不标记，下次启动可重试。清理旧源在校验通过后执行，
 * 即使清理本身异常也不影响 done 标记（数据已确认落库）。
 *
 * @param summary 检测阶段得到的计数汇总
 * @param onProgress 可选进度回调
 */
export const migrateLegacyData = async (
  summary: LegacyDataSummary,
  onProgress?: (progress: MigrationProgress) => void,
): Promise<void> => {
  const ledger = readLedger();

  if (ledger.tasks === "pending" && summary.tasks > 0) {
    onProgress?.({ domain: "tasks", done: 0, total: summary.tasks });
    const sanitized = sanitizeLegacyTasks(await readLegacyTasks());
    if (sanitized.length > 0) {
      await invoke("db_upsert_tasks_batch", { tasks: sanitized });
      const landed = await verifyTasksLanded(sanitized.map((task) => task.id));
      if (!landed) throw new Error("tasks verification failed");
    }
    onProgress?.({ domain: "tasks", done: summary.tasks, total: summary.tasks });
    await clearLegacyTasks();
    ledger.tasks = "done";
    writeLedger(ledger);
  }

  if (ledger.history === "pending" && summary.history > 0) {
    onProgress?.({ domain: "history", done: 0, total: summary.history });
    const items = readLegacyHistory();
    if (items.length > 0) {
      await invoke("db_add_history_batch", { items });
      const landed = await verifyHistoryLanded(items.map((item) => item.url));
      if (!landed) throw new Error("history verification failed");
    }
    onProgress?.({ domain: "history", done: summary.history, total: summary.history });
    clearLegacyHistory();
    ledger.history = "done";
    writeLedger(ledger);
  }
};
