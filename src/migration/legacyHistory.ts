import type { HistoryItem } from "@/stores/history";

/**
 * 老版本解析历史数据源（localStorage）的读取与清洗
 *
 * 读取/解析是纯函数，副作用（落库/清 key/标记）由 runner 编排。
 */

/** 老版本历史的存储 key（pinia 持久化，值为 { items: [...] } 或数组） */
export const LEGACY_HISTORY_KEY = "history";

type RawHistoryEntry = { url?: string; title?: string; time?: number } | string;

const hasStorage = (): boolean => typeof window !== "undefined" && !!window.localStorage;

/**
 * 解析老版本历史原始字符串（纯函数）
 *
 * 兼容三种历史格式：{ items: [...] } / [...] / { urls: [...] }，
 * 以及字符串条目（url 即标题）。
 *
 * @param raw localStorage 原始字符串，无数据传 null
 * @returns 有效历史列表
 */
export const parseLegacyHistory = (raw: string | null): HistoryItem[] => {
  if (!raw) return [];
  try {
    const parsed = JSON.parse(raw);
    let rawList: RawHistoryEntry[] = [];

    if (Array.isArray(parsed?.items)) {
      rawList = parsed.items;
    } else if (Array.isArray(parsed)) {
      rawList = parsed;
    } else if (Array.isArray(parsed?.urls)) {
      rawList = parsed.urls;
    }

    return rawList
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
  } catch {
    return [];
  }
};

/** 仅统计老版本历史条数（弹窗计数用） */
export const countLegacyHistory = (): number => {
  if (!hasStorage()) return 0;
  return parseLegacyHistory(localStorage.getItem(LEGACY_HISTORY_KEY)).length;
};

/** 读取老版本历史全量（已清洗） */
export const readLegacyHistory = (): HistoryItem[] => {
  if (!hasStorage()) return [];
  return parseLegacyHistory(localStorage.getItem(LEGACY_HISTORY_KEY));
};

/**
 * 清空老版本历史数据源
 *
 * 仅在迁移成功并校验通过后由 runner 调用；平时不调用。
 */
export const clearLegacyHistory = (): void => {
  if (!hasStorage()) return;
  localStorage.removeItem(LEGACY_HISTORY_KEY);
};
