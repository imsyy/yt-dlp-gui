import { defineStore } from "pinia";
import { invoke } from "@tauri-apps/api/core";

const MAX_HISTORY = 50;

/** 解析历史单项记录 */
export interface HistoryItem {
  url: string;
  title: string;
  time: number;
}

/**
 * 视频解析历史 Store
 */
export const useHistoryStore = defineStore("history", () => {
  const items = ref<HistoryItem[]>([]);
  const loaded = ref(false);

  const urls = computed(() => items.value.map((i) => i.url));

  /**
   * 从后端 SQLite 加载解析历史
   *
   * 老版本数据迁移由启动弹窗（MigrationModal）显式触发，迁移完成后会调用
   * 本函数重载；此处只读库，不做任何迁移。
   *
   * @returns 加载结果 Promise
   */
  const loadHistory = async (): Promise<void> => {
    try {
      items.value = await invoke<HistoryItem[]>("db_get_history");
    } catch (error) {
      console.error("加载解析历史失败:", error);
    } finally {
      loaded.value = true;
    }
  };

  loadHistory();

  /**
   * 添加一条成功获取过的链接（内存去重，最新在前）并异步写入 SQLite
   *
   * @param url 解析成功的视频或播放列表链接
   * @param title 视频或播放列表标题，不传则回退为 url
   */
  const add = (url: string, title?: string): void => {
    const trimmed = url.trim();
    if (!trimmed) return;
    const finalTitle = title?.trim() || trimmed;
    const now = Date.now();

    const idx = items.value.findIndex((i) => i.url === trimmed);
    if (idx !== -1) items.value.splice(idx, 1);
    items.value.unshift({ url: trimmed, title: finalTitle, time: now });
    if (items.value.length > MAX_HISTORY) items.value.length = MAX_HISTORY;

    invoke("db_add_history", { url: trimmed, title: finalTitle }).catch((err) => {
      console.warn("保存解析历史至数据库失败:", err);
    });
  };

  /**
   * 删除单条解析记录并同步从 SQLite 中移除
   *
   * @param url 需要删除的链接
   */
  const remove = (url: string): void => {
    const idx = items.value.findIndex((i) => i.url === url);
    if (idx !== -1) items.value.splice(idx, 1);

    invoke("db_remove_history", { url }).catch((err) => {
      console.warn("从数据库移除解析历史失败:", err);
    });
  };

  /**
   * 清空全部解析历史并同步清空 SQLite 对应表
   */
  const clear = (): void => {
    items.value = [];
    invoke("db_clear_history").catch((err) => {
      console.warn("清空数据库解析历史失败:", err);
    });
  };

  return { items, urls, loaded, loadHistory, add, remove, clear };
});
