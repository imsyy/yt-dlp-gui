import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type ToolTaskStatus = "running" | "completed" | "failed" | "cancelled" | "interrupted";

export interface ToolTaskState {
  toolId: string;
  runId: string;
  status: ToolTaskStatus;
  url: string;
  stage: string | null;
  progress: number | null;
  error: string | null;
  startedAt: number;
  updatedAt: number;
}

export interface ToolResultRecord {
  toolId: string;
  runId: string;
  resultType: "json" | "paged";
  resultJson: string | null;
  total: number;
  completedAt: number;
}

/**
 * 工具页只观察 Rust 后台任务：状态通过事件同步，结果始终独立读取。
 * 页面卸载只解除监听，不会取消后台任务。
 */
export const useToolTask = <T>(toolId: string) => {
  const state = ref<ToolTaskState | null>(null);
  const resultMeta = ref<ToolResultRecord | null>(null);
  const result = shallowRef<T | null>(null);
  let unlisten: UnlistenFn | null = null;

  const running = computed(() => state.value?.status === "running");

  const loadResult = async () => {
    const record = await invoke<ToolResultRecord | null>("tool_get_result", { toolId });
    resultMeta.value = record;
    if (record?.resultType === "json" && record.resultJson) {
      result.value = JSON.parse(record.resultJson) as T;
    }
    return record;
  };

  const refresh = async () => {
    state.value = await invoke<ToolTaskState | null>("tool_get_task_state", { toolId });
    await loadResult();
  };

  const start = async (url: string, params: Record<string, unknown>) => {
    state.value = await invoke<ToolTaskState>("tool_start_task", {
      toolId,
      url,
      params,
    });
  };

  onMounted(async () => {
    unlisten = await listen<ToolTaskState>("tool-task-state-changed", async ({ payload }) => {
      if (payload.toolId !== toolId) return;
      state.value = payload;
      if (payload.status === "completed") await loadResult();
    });
    await refresh();
  });

  onUnmounted(() => {
    unlisten?.();
    unlisten = null;
  });

  return { state, result, resultMeta, running, start, refresh, loadResult };
};
