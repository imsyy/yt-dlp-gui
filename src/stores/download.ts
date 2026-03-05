import { defineStore } from "pinia";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import localforage from "localforage";
import type { DownloadTask } from "@/types";

const storage = localforage.createInstance({
  name: "yt-dlp-gui",
  storeName: "downloads",
});

const STORAGE_KEY = "download_tasks";

interface ProgressPayload {
  id: string;
  percent: number;
  speed: string;
  eta: string;
  downloaded: string;
  total: string;
}

export const useDownloadStore = defineStore("download", () => {
  const tasks = ref<DownloadTask[]>([]);
  const loaded = ref(false);
  let listenersSetup = false;
  let saveTimer: ReturnType<typeof setTimeout> | null = null;

  // ========== 持久化 ==========

  /** 防抖保存任务列表到 IndexedDB，延迟 500ms */
  const saveTasks = () => {
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(() => {
      storage.setItem(STORAGE_KEY, JSON.parse(JSON.stringify(tasks.value)));
    }, 500);
  };

  /** 从 IndexedDB 恢复任务列表，将之前未完成的任务标记为中断 */
  const loadTasks = async () => {
    const saved = await storage.getItem<DownloadTask[]>(STORAGE_KEY);
    if (saved && Array.isArray(saved)) {
      for (const task of saved) {
        if (task.status === "downloading" || task.status === "paused") {
          task.status = "error";
          task.error = "应用重启，下载已中断";
          task.speed = "";
        }
        if (!Array.isArray(task.logs)) task.logs = [];
        if (!task.createdAt) task.createdAt = Date.now();
      }
      tasks.value = saved;
    }
    loaded.value = true;
  };

  watch(tasks, saveTasks, { deep: true });

  // ========== 事件监听 ==========

  /** 注册 Tauri 后端事件监听（进度、日志、完成、错误），仅初始化一次 */
  const setupListeners = async () => {
    if (listenersSetup) return;
    listenersSetup = true;

    await listen<ProgressPayload>("download-progress", (event) => {
      const task = tasks.value.find((t) => t.id === event.payload.id);
      if (task && task.status === "downloading") {
        task.percent = event.payload.percent;
        task.speed = event.payload.speed;
        task.eta = event.payload.eta;
        if (event.payload.downloaded) task.downloaded = event.payload.downloaded;
        if (event.payload.total) task.total = event.payload.total;
      }
    });

    await listen<{ id: string; line: string }>("download-log", (event) => {
      const task = tasks.value.find((t) => t.id === event.payload.id);
      if (task) {
        task.logs.push(event.payload.line);
      }
    });

    await listen<{ id: string; outputFile: string }>("download-complete", (event) => {
      const task = tasks.value.find((t) => t.id === event.payload.id);
      if (task) {
        task.status = "completed";
        task.percent = 100;
        task.speed = "";
        if (event.payload.outputFile) task.outputFile = event.payload.outputFile;
      }
    });

    await listen<{ id: string; error: string }>("download-error", (event) => {
      const task = tasks.value.find((t) => t.id === event.payload.id);
      if (task && task.status !== "cancelled") {
        task.status = "error";
        task.error = event.payload.error;
        task.speed = "";
      }
    });
  };

  // Auto-init
  loadTasks();
  setupListeners();

  // ========== 操作 ==========

  /** 添加新的下载任务到列表顶部 */
  const addTask = (task: DownloadTask) => {
    tasks.value.unshift(task);
  };

  /** 暂停指定下载任务，通过 Tauri 命令挂起后端进程 */
  const pauseTask = async (id: string) => {
    await invoke("pause_download", { id });
    const task = tasks.value.find((t) => t.id === id);
    if (task) {
      task.status = "paused";
      task.speed = "";
    }
  };

  /** 恢复指定已暂停的下载任务 */
  const resumeTask = async (id: string) => {
    await invoke("resume_download", { id });
    const task = tasks.value.find((t) => t.id === id);
    if (task) {
      task.status = "downloading";
    }
  };

  /** 取消下载任务并删除已下载的文件 */
  const cancelTask = async (id: string) => {
    const task = tasks.value.find((t) => t.id === id);
    if (task) task.status = "cancelled";
    try {
      await invoke("cancel_download", { id, deleteFiles: true });
    } catch {
      // Process might have already exited
    }
  };

  /** 重新下载失败或已取消的任务，生成新 ID 并重置状态 */
  const retryTask = async (id: string) => {
    const task = tasks.value.find((t) => t.id === id);
    if (!task) return;

    const newId = `dl_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`;

    task.id = newId;
    task.status = "downloading";
    task.percent = 0;
    task.speed = "";
    task.eta = "";
    task.downloaded = "";
    task.total = "";
    task.logs = [];
    task.error = undefined;

    await invoke("start_download", {
      params: { id: newId, ...task.params },
    });
  };

  /** 从列表中移除指定任务 */
  const removeTask = (id: string) => {
    const idx = tasks.value.findIndex((t) => t.id === id);
    if (idx !== -1) tasks.value.splice(idx, 1);
  };

  /** 清空所有已完成、失败、已取消的任务 */
  const clearFinished = () => {
    tasks.value = tasks.value.filter(
      (t) =>
        t.status !== "completed" &&
        t.status !== "error" &&
        t.status !== "cancelled",
    );
  };

  return {
    tasks,
    loaded,
    addTask,
    pauseTask,
    resumeTask,
    cancelTask,
    retryTask,
    removeTask,
    clearFinished,
  };
});
