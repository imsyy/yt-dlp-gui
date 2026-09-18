import { invoke } from "@tauri-apps/api/core";
import type { Router } from "vue-router";

interface ToolSnapshotRow {
  url: string;
  title: string;
  resultJson: string;
}

export interface ToolSnapshot<T> {
  url: string;
  title: string;
  payload: T;
}

/** 读取某工具唯一的当前结果快照 */
export const loadToolSnapshot = async <T>(tool: string): Promise<ToolSnapshot<T> | null> => {
  try {
    const row = await invoke<ToolSnapshotRow | null>("db_get_tool_snapshot", { tool });
    if (!row) return null;
    return { url: row.url, title: row.title, payload: JSON.parse(row.resultJson) as T };
  } catch (error) {
    console.warn("[YDL GUI] 读取工具快照失败:", error);
    return null;
  }
};

/** 覆盖保存某工具的当前结果 */
export const saveToolSnapshot = async (
  tool: string,
  url: string,
  title: string,
  payload: unknown,
): Promise<void> => {
  try {
    await invoke("db_save_tool_snapshot", {
      tool,
      url,
      title,
      resultJson: JSON.stringify(payload),
    });
  } catch (error) {
    console.warn("[YDL GUI] 保存工具快照失败:", error);
  }
};

/**
 * 返回工具列表
 */
export const goToolList = (router: Router): void => {
  const state = window.history.state as { back?: unknown } | null;
  if (state?.back) router.back();
  else void router.push({ name: "toolbox" });
};
