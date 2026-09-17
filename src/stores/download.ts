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
import type { DownloadTask, DownloadTaskParams } from "@/types";
import { useSettingStore } from "@/stores/setting";
import { formatFileSize } from "@/utils/format";
import { cleanMultipleTasksResiduals } from "@/utils/taskFiles";
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

  /** 补齐下载参数默认值，防止历史数据字段缺失导致后端反序列化报错 */
  const ensureCompleteParams = (params: Partial<DownloadTaskParams>): DownloadTaskParams => ({
    url: params.url || "",
    downloadDir: params.downloadDir || "",
    downloadMode: params.downloadMode || "default",
    videoFormat: params.videoFormat ?? null,
    audioFormat: params.audioFormat ?? null,
    cookieFile: params.cookieFile ?? null,
    cookieBrowser: params.cookieBrowser ?? null,
    proxy: params.proxy ?? null,
    outputTemplate: params.outputTemplate ?? null,
    concurrentFragments: params.concurrentFragments ?? null,
    noOverwrites: Boolean(params.noOverwrites),
    embedSubs: Boolean(params.embedSubs),
    embedThumbnail: Boolean(params.embedThumbnail),
    writeThumbnail: Boolean(params.writeThumbnail),
    writeDescription: Boolean(params.writeDescription),
    embedMetadata: Boolean(params.embedMetadata),
    embedChapters: Boolean(params.embedChapters),
    sponsorblockRemove: Boolean(params.sponsorblockRemove),
    extractAudio: Boolean(params.extractAudio),
    audioConvertFormat: params.audioConvertFormat ?? null,
    noMerge: Boolean(params.noMerge),
    recodeFormat: params.recodeFormat ?? null,
    remuxFormat: params.remuxFormat ?? null,
    limitRate: params.limitRate ?? null,
    ffmpegArgs: params.ffmpegArgs ?? null,
    customArgs: params.customArgs ?? null,
    subtitles: Array.isArray(params.subtitles) ? params.subtitles : [],
    startTime: params.startTime ?? null,
    endTime: params.endTime ?? null,
    noPlaylist: Boolean(params.noPlaylist),
    playlistItems: params.playlistItems ?? null,
    liveFromStart: Boolean(params.liveFromStart),
  });

  /** 尝试启动队列中的下一个任务 */
  const tryStartNext = async () => {
    const settingStore = useSettingStore();
    const max = settingStore.maxConcurrentDownloads;
    if (max > 0 && activeCount.value >= max) return;

    const next = tasks.value.find((taskItem) => taskItem.status === "queued");
    if (!next) return;

    const fullParams = ensureCompleteParams(next.params);
    next.params = fullParams;
    next.status = "downloading";
    try {
      await invoke("start_download", {
        params: { id: next.id, ...fullParams },
      });
    } catch (error: unknown) {
      next.status = "error";
      next.error =
        error instanceof Error
          ? error.message
          : String(error) || i18n.global.t("downloads.startFailed");
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
        // 兼容老版本数据中可能存在的 paused 状态，归一为 error
        if ((task.status as string) === "paused") {
          task.status = "error";
          task.error = i18n.global.t("downloads.appRestarted");
          task.speed = "";
        }
        if (
          task.status === "preparing" ||
          task.status === "downloading" ||
          task.status === "postprocessing" ||
          task.status === "queued"
        ) {
          task.status = "error";
          task.error = i18n.global.t("downloads.appRestarted");
          task.speed = "";
        }
        // 已完成的任务清理并清空 logs，避免占用本地存储
        if (task.status === "completed") {
          task.logs = [];
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

    if (downloading.length > 0) {
      const avg = Math.round(
        downloading.reduce((sum, t) => sum + (t.percent || 0), 0) / downloading.length,
      );
      appWindow.setProgressBar({ status: ProgressBarStatus.Normal, progress: avg });
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

    await listen<{ id: string; outputFile: string; fileSizeBytes?: number }>(
      "download-complete",
      (event) => {
        const task = tasks.value.find((t) => t.id === event.payload.id);
        if (task) {
          task.status = "completed";
          task.percent = 100;
          task.speed = "";
          task.logs = []; // 完成的任务清空日志，避免占用 IndexedDB 存储空间
          if (event.payload.outputFile) task.outputFile = event.payload.outputFile;
          if (typeof event.payload.fileSizeBytes === "number" && event.payload.fileSizeBytes > 0) {
            task.fileSizeBytes = event.payload.fileSizeBytes;
            task.total = formatFileSize(event.payload.fileSizeBytes);
          }
          notify(
            i18n.global.t("downloads.notifyComplete"),
            task.title || i18n.global.t("downloads.notifyCompleteBody"),
          );
        }
        updateTaskbarProgress();
        tryStartNext();
      },
    );

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

  /** 取消下载任务（终止进程并移至已取消，不删除文件） */
  const cancelTask = async (id: string) => {
    const task = tasks.value.find((t) => t.id === id);
    if (!task) return;

    const hadNoProcess = task.status === "queued" || task.status === "preparing";
    task.status = "cancelled";

    if (!hadNoProcess) {
      try {
        await invoke("cancel_download", { id, deleteFiles: false });
      } catch {
        // Process might have already exited
      }
    }

    updateTaskbarProgress();
    tryStartNext();
  };

  /** 重新下载失败或已取消的任务（原位重试） */
  const retryTask = async (id: string) => {
    const task = tasks.value.find((taskItem) => taskItem.id === id);
    if (!task) return;

    const newId = `dl_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`;
    const fullParams = ensureCompleteParams(task.params);
    task.id = newId;
    task.params = fullParams;
    task.percent = 0;
    task.speed = "";
    task.eta = "";
    task.downloaded = "";
    task.total = "";
    task.logs = [];
    task.error = undefined;

    if (canStartNow()) {
      task.status = "downloading";
      try {
        await invoke("start_download", {
          params: { id: newId, ...fullParams },
        });
      } catch (error: unknown) {
        task.status = "error";
        task.error =
          error instanceof Error
            ? error.message
            : String(error) || i18n.global.t("downloads.startFailed");
      }
    } else {
      task.status = "queued";
    }
  };

  /**
   * 针对已完成任务的「重新下载」：
   * 绝不篡改原有的已完成历史记录，克隆配置生成新任务压入队列
   */
  const reDownloadTask = async (id: string) => {
    const existingTask = tasks.value.find((taskItem) => taskItem.id === id);
    if (!existingTask) return;

    const newTaskId = `dl_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`;
    const fullParams = ensureCompleteParams(existingTask.params);

    const newTask: DownloadTask = {
      id: newTaskId,
      url: existingTask.url,
      title: existingTask.title,
      thumbnail: existingTask.thumbnail,
      formatLabel: existingTask.formatLabel,
      status: "queued",
      percent: 0,
      speed: "",
      eta: "",
      downloaded: "",
      total: "",
      logs: [],
      createdAt: Date.now(),
      params: fullParams,
    };

    tasks.value.unshift(newTask);

    if (canStartNow()) {
      newTask.status = "downloading";
      try {
        await invoke("start_download", {
          params: { id: newTaskId, ...fullParams },
        });
      } catch (error: unknown) {
        newTask.status = "error";
        newTask.error =
          error instanceof Error
            ? error.message
            : String(error) || i18n.global.t("downloads.startFailed");
      }
    }
  };

  /** 重新尝试所有失败与已取消的任务 */
  const retryAllInterrupted = async () => {
    const targetTasks = tasks.value.filter(
      (task) => task.status === "error" || task.status === "cancelled",
    );
    for (const targetTask of targetTasks) {
      await retryTask(targetTask.id);
    }
  };

  /** 从列表中移除指定任务 */
  const removeTask = (id: string) => {
    const idx = tasks.value.findIndex((taskItem) => taskItem.id === id);
    if (idx !== -1) tasks.value.splice(idx, 1);
  };

  /** 仅清空所有已成功完成的任务 */
  const clearCompleted = () => {
    tasks.value = tasks.value.filter((task) => task.status !== "completed");
  };

  /** 仅清空已取消的任务（同时清理磁盘残留临时文件） */
  const clearCancelled = async () => {
    const cancelledTasks = tasks.value.filter((task) => task.status === "cancelled");
    tasks.value = tasks.value.filter((task) => task.status !== "cancelled");
    if (cancelledTasks.length > 0) {
      await cleanMultipleTasksResiduals(cancelledTasks);
    }
  };

  /** 仅清空下载失败的任务（同时清理磁盘残留临时文件） */
  const clearFailed = async () => {
    const failedTasks = tasks.value.filter((task) => task.status === "error");
    tasks.value = tasks.value.filter((task) => task.status !== "error");
    if (failedTasks.length > 0) {
      await cleanMultipleTasksResiduals(failedTasks);
    }
  };

  /** 仅清空所有失败与已取消的任务（同时清理磁盘残留临时文件） */
  const clearInterrupted = async () => {
    const interruptedTasks = tasks.value.filter(
      (task) => task.status === "error" || task.status === "cancelled",
    );
    tasks.value = tasks.value.filter(
      (task) => task.status !== "error" && task.status !== "cancelled",
    );
    if (interruptedTasks.length > 0) {
      await cleanMultipleTasksResiduals(interruptedTasks);
    }
  };

  return {
    tasks,
    loaded,
    activeCount,
    canStartNow,
    addTask,
    cancelTask,
    retryTask,
    reDownloadTask,
    retryAllInterrupted,
    removeTask,
    clearCompleted,
    clearCancelled,
    clearFailed,
    clearInterrupted,
  };
});
