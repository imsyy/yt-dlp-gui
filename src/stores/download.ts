import { defineStore } from "pinia";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow, ProgressBarStatus } from "@tauri-apps/api/window";
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";
import localforage from "localforage";
import type { DownloadTask } from "@/types";
import { useSettingStore } from "@/stores/setting";
import i18n from "@/locales";

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
  fragmentIndex?: number;
  fragmentCount?: number;
  status?: string;
}

export const useDownloadStore = defineStore("download", () => {
  const tasks = ref<DownloadTask[]>([]);
  const loaded = ref(false);
  let listenersSetup = false;
  let saveTimer: ReturnType<typeof setTimeout> | null = null;

  /** 当前占用下载进程的任务数（后处理尚未释放进程，也计入并发槽位） */
  const activeCount = computed(
    () =>
      tasks.value.filter(
        (task) => task.status === "downloading" || task.status === "postprocessing",
      ).length,
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
      next.error = i18n.global.t("downloads.startFailed");
    }
  };

  /** 判断是否需要排队，返回 true 表示可以直接下载 */
  const canStartNow = (): boolean => {
    const settingStore = useSettingStore();
    const max = settingStore.maxConcurrentDownloads;
    return max <= 0 || activeCount.value < max;
  };

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

  /** 防抖保存任务列表到 IndexedDB */
  const saveTasks = () => {
    if (!loaded.value) return;
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
        if (
          task.status === "downloading" ||
          task.status === "postprocessing" ||
          task.status === "paused" ||
          task.status === "queued"
        ) {
          task.status = "error";
          task.error = i18n.global.t("downloads.appRestarted");
          task.speed = "";
        }
        if (!Array.isArray(task.logs)) task.logs = [];
        if (!task.createdAt) task.createdAt = Date.now();
      }

      // Filter out completed tasks whose output files no longer exist
      const completedWithFile = saved.filter((t) => t.status === "completed" && t.outputFile);
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

  /** 更新任务栏进度条 */
  const updateTaskbarProgress = () => {
    const settingStore = useSettingStore();
    const appWindow = getCurrentWindow();

    if (!settingStore.showTaskbarProgress) {
      appWindow.setProgressBar({ status: ProgressBarStatus.None });
      return;
    }

    const downloading = tasks.value.filter((t) => t.status === "downloading");
    const paused = tasks.value.filter((t) => t.status === "paused");

    if (downloading.length > 0) {
      const avg = Math.round(
        downloading.reduce((sum, t) => sum + (t.percent || 0), 0) / downloading.length,
      );
      appWindow.setProgressBar({ status: ProgressBarStatus.Normal, progress: avg });
    } else if (paused.length > 0) {
      const avg = Math.round(paused.reduce((sum, t) => sum + (t.percent || 0), 0) / paused.length);
      appWindow.setProgressBar({ status: ProgressBarStatus.Paused, progress: avg });
    } else {
      appWindow.setProgressBar({ status: ProgressBarStatus.None });
    }
  };

  /** 注册 Tauri 后端事件监听，仅初始化一次 */
  const setupListeners = async () => {
    if (listenersSetup) return;
    listenersSetup = true;

    await listen<ProgressPayload>("download-progress", (event) => {
      const task = tasks.value.find((t) => t.id === event.payload.id);
      if (task && (task.status === "downloading" || task.status === "postprocessing")) {
        const status = event.payload.status;
        if (status === "postprocessing") {
          // 后处理阶段没有网络下载速度/ETA，切换阶段时清掉上一阶段数据。
          task.status = "postprocessing";
          task.speed = "";
          task.eta = "";
          if (event.payload.percent > 0) task.percent = event.payload.percent;
          if (event.payload.downloaded) task.downloaded = event.payload.downloaded;
          if (event.payload.total) task.total = event.payload.total;
        } else {
          task.status = "downloading";
          task.percent = event.payload.percent;
          task.speed = event.payload.speed;
          task.eta = event.payload.eta;
          // 分片切换时 yt-dlp 偶尔会暂时不给大小，保留最近一次有效值以避免闪烁。
          if (event.payload.downloaded) task.downloaded = event.payload.downloaded;
          if (event.payload.total) task.total = event.payload.total;
        }
      }
      updateTaskbarProgress();
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
        notify(
          i18n.global.t("downloads.notifyComplete"),
          task.title || i18n.global.t("downloads.notifyCompleteBody"),
        );
      }
      updateTaskbarProgress();
      tryStartNext();
    });

    await listen<{ id: string; error: string }>("download-error", (event) => {
      const task = tasks.value.find((t) => t.id === event.payload.id);
      if (task && task.status !== "cancelled") {
        task.status = "error";
        task.error = event.payload.error;
        task.speed = "";
      }
      updateTaskbarProgress();
      tryStartNext();
    });
  };

  loadTasks();
  setupListeners();

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
    updateTaskbarProgress();
  };

  /** 恢复指定已暂停的下载任务 */
  const resumeTask = async (id: string) => {
    await invoke("resume_download", { id });
    const task = tasks.value.find((t) => t.id === id);
    if (task) {
      task.status = "downloading";
    }
    updateTaskbarProgress();
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

    updateTaskbarProgress();
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
      (t) => t.status !== "completed" && t.status !== "error" && t.status !== "cancelled",
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
