import { defineStore } from "pinia";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";
import localforage from "localforage";
import type { DownloadTask } from "@/types";
import { useSettingStore } from "@/stores/setting";

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

  // ========== 队列 ==========

  /** 当前正在下载的任务数 */
  const activeCount = computed(
    () => tasks.value.filter((t) => t.status === "downloading").length,
  );

  /** 尝试启动队列中的下一个任务 */
  const tryStartNext = async () => {
    const settingStore = useSettingStore();
    const max = settingStore.maxConcurrentDownloads;
    if (max > 0 && activeCount.value >= max) return;

    const next = tasks.value.find((t) => t.status === "queued");
    if (!next) return;

    next.status = "downloading";
    try {
      await invoke("start_download", {
        params: { id: next.id, ...next.params },
      });
    } catch {
      next.status = "error";
      next.error = "启动下载失败";
    }
  };

  /** 判断是否需要排队，返回 true 表示可以直接下载 */
  const canStartNow = (): boolean => {
    const settingStore = useSettingStore();
    const max = settingStore.maxConcurrentDownloads;
    return max <= 0 || activeCount.value < max;
  };

  // ========== 通知 ==========

  const notify = async (title: string, body: string) => {
    const settingStore = useSettingStore();
    const mode = settingStore.notifyMode;
    if (mode === "none") return;

    if (mode === "app" || mode === "all") {
      window.$notification.create({ title, content: body, duration: 5000 });
    }

    if (mode === "system" || mode === "all") {
      let granted = await isPermissionGranted();
      if (!granted) {
        const permission = await requestPermission();
        granted = permission === "granted";
      }
      if (granted) {
        sendNotification({ title, body });
      }
    }
  };

  // ========== 持久化 ==========

  /** 防抖保存任务列表到 IndexedDB，延迟 500ms */
  const saveTasks = () => {
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(() => {
      storage.setItem(STORAGE_KEY, JSON.parse(JSON.stringify(tasks.value)));
    }, 500);
  };

  /** 从 IndexedDB 恢复任务列表，将之前未完成的任务标记为中断，移除文件已不存在的已完成任务 */
  const loadTasks = async () => {
    const saved = await storage.getItem<DownloadTask[]>(STORAGE_KEY);
    if (saved && Array.isArray(saved)) {
      for (const task of saved) {
        if (task.status === "downloading" || task.status === "paused" || task.status === "queued") {
          task.status = "error";
          task.error = "应用重启，下载已中断";
          task.speed = "";
        }
        if (!Array.isArray(task.logs)) task.logs = [];
        if (!task.createdAt) task.createdAt = Date.now();
      }

      // Filter out completed tasks whose output files no longer exist
      const completedWithFile = saved.filter(
        (t) => t.status === "completed" && t.outputFile,
      );
      if (completedWithFile.length > 0) {
        try {
          const paths = completedWithFile.map((t) => t.outputFile!);
          const exists = await invoke<boolean[]>("check_files_exist", { paths });
          const missingIds = new Set<string>();
          completedWithFile.forEach((t, i) => {
            if (!exists[i]) missingIds.add(t.id);
          });
          if (missingIds.size > 0) {
            const filtered = saved.filter((t) => !missingIds.has(t.id));
            tasks.value = filtered;
            loaded.value = true;
            return;
          }
        } catch {
          // If check fails, keep all tasks
        }
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
        notify("下载完成", task.title || "视频下载已完成");
      }
      // 下载完成后尝试启动队列中的下一个
      tryStartNext();
    });

    await listen<{ id: string; error: string }>("download-error", (event) => {
      const task = tasks.value.find((t) => t.id === event.payload.id);
      if (task && task.status !== "cancelled") {
        task.status = "error";
        task.error = event.payload.error;
        task.speed = "";
      }
      // 出错后尝试启动队列中的下一个
      tryStartNext();
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
    if (!task) return;

    const wasQueued = task.status === "queued";
    task.status = "cancelled";

    if (!wasQueued) {
      try {
        await invoke("cancel_download", { id, deleteFiles: true });
      } catch {
        // Process might have already exited
      }
    }

    // 取消后尝试启动队列中的下一个
    tryStartNext();
  };

  /** 重新下载失败或已取消的任务，生成新 ID 并重置状态 */
  const retryTask = async (id: string) => {
    const task = tasks.value.find((t) => t.id === id);
    if (!task) return;

    const newId = `dl_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`;
    task.id = newId;
    task.percent = 0;
    task.speed = "";
    task.eta = "";
    task.downloaded = "";
    task.total = "";
    task.logs = [];
    task.error = undefined;

    if (canStartNow()) {
      task.status = "downloading";
      await invoke("start_download", {
        params: { id: newId, ...task.params },
      });
    } else {
      task.status = "queued";
    }
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
    activeCount,
    canStartNow,
    addTask,
    pauseTask,
    resumeTask,
    cancelTask,
    retryTask,
    removeTask,
    clearFinished,
  };
});
