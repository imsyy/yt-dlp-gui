import { defineStore } from "pinia";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow, ProgressBarStatus } from "@tauri-apps/api/window";
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";
import type { DownloadTask, DownloadTaskParams } from "@/types";
import { useSettingStore } from "@/stores/setting";
import { formatFileSize } from "@/utils/format";
import { cleanMultipleTasksResiduals } from "@/utils/taskFiles";
import { normalizeTaskParams } from "@/utils/taskParams";
import { pushTaskLog, trimTaskLogs } from "@/utils/logs";
import i18n from "@/locales";

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

/**
 * 视频下载管理 Store
 */
export const useDownloadStore = defineStore("download", () => {
  const settingStore = useSettingStore();
  const tasks = ref<DownloadTask[]>([]);
  const loaded = ref(false);
  let listenersSetup = false;
  const pendingTaskIds = new Set<string>();
  const pendingWrites = new Map<string, Promise<boolean>>();
  const startingTasks = new Map<string, Promise<unknown>>();
  const startingIds = reactive(new Set<string>());
  const cancellingIds = reactive(new Set<string>());
  const cancellations = new Map<string, Promise<void>>();
  const retryingTasks = new Set<DownloadTask>();
  let scheduling: Promise<void> | undefined;
  let scheduleRequested = false;

  /** 当前占用下载进程的任务数（后处理尚未释放进程，也计入并发槽位） */
  const activeCount = computed(() => {
    const ids = new Set([...startingIds, ...cancellingIds]);
    for (const task of tasks.value) {
      if (task.status === "downloading" || task.status === "postprocessing") ids.add(task.id);
    }
    return ids.size;
  });

  const errorMessage = (error: unknown): string =>
    error instanceof Error
      ? error.message
      : String(error) || i18n.global.t("downloads.startFailed");

  /**
   * 统一启动已落库的排队任务，填满空闲槽位；并发调用共享同一轮调度。
   *
   * @returns 启动流程 Promise
   */
  const tryStartNext = async (): Promise<void> => {
    scheduleRequested = true;
    if (scheduling) return scheduling;

    scheduling = (async () => {
      await ready;
      do {
        scheduleRequested = false;
        while (activeCount.value < settingStore.maxConcurrent) {
          const next = tasks.value.find(
            (task) => task.status === "queued" && !pendingTaskIds.has(task.id),
          );
          if (!next) break;

          const id = next.id;
          next.params = normalizeTaskParams(next.params);
          next.status = "downloading";
          startingIds.add(id);
          const starting = invoke("start_download", { params: { id, ...next.params } });
          startingTasks.set(id, starting);
          try {
            await starting;
          } catch (error: unknown) {
            // A cancellation or a terminal event may already have changed the task.
            if (next.id === id && next.status === "downloading" && tasks.value.includes(next)) {
              next.status = "error";
              next.error = errorMessage(error);
              next.speed = "";
              next.eta = "";
              await updateTask(next);
            }
          } finally {
            startingTasks.delete(id);
            startingIds.delete(id);
          }
        }
      } while (scheduleRequested);
    })().finally(() => {
      scheduling = undefined;
      // A request can arrive after the last scan but before this promise settles.
      if (scheduleRequested) void tryStartNext();
    });
    return scheduling;
  };

  /**
   * 发送应用内或系统级桌面通知
   *
   * @param title 通知标题
   * @param body 通知正文内容
   */
  const notify = async (title: string, body: string): Promise<void> => {
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

  /**
   * 从后端 SQLite 加载任务列表并检验已完成任务的文件有效性
   *
   * 老版本数据迁移由启动弹窗（MigrationModal）显式触发，迁移完成后会调用
   * 本函数重载；此处只读库，不做任何迁移。
   * 后端冷启动时已将异常退出的进行中任务归一化为错误中断态。
   *
   * @returns 加载任务完成 Promise
   */
  const loadTasks = async (): Promise<void> => {
    try {
      const dbTasks = await invoke<DownloadTask[]>("db_get_tasks");
      // 老版本迁移来的日志可能是无界的，统一裁剪到环形缓冲上限
      for (const t of dbTasks) {
        if (Array.isArray(t.logs)) trimTaskLogs(t.logs);
        else t.logs = [];
      }
      tasks.value = dbTasks;

      // 异步检验已完成任务的输出文件是否依然存在于磁盘上，不存在则自动清理
      const completedWithFile = tasks.value.filter((t) => t.status === "completed" && t.outputFile);
      if (completedWithFile.length > 0) {
        try {
          const paths = completedWithFile.map((t) => t.outputFile!);
          const exists = await invoke<boolean[]>("check_files_exist", { paths });
          const missingIds: string[] = [];
          completedWithFile.forEach((t, i) => {
            if (!exists[i]) missingIds.push(t.id);
          });
          if (missingIds.length > 0) {
            await invoke("db_delete_tasks", { ids: missingIds });
            tasks.value = tasks.value.filter((t) => !missingIds.includes(t.id));
          }
        } catch {
          // 忽略失效文件检查异常
        }
      }
    } catch (error) {
      console.error("从 SQLite 加载任务列表失败:", error);
    } finally {
      loaded.value = true;
    }
  };

  /**
   * 刷新系统任务栏整体下载进度条状态
   */
  const updateTaskbarProgress = (): void => {
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

  /**
   * 注册 Tauri 后端事件监听（进度、日志、完成、失败），应用生命周期内仅初始化一次
   *
   * @returns 监听初始化 Promise
   */
  const setupListeners = async (): Promise<void> => {
    if (listenersSetup) return;
    listenersSetup = true;

    await listen<ProgressPayload>("download-progress", (event) => {
      const task = tasks.value.find((t) => t.id === event.payload.id);
      if (task && (task.status === "downloading" || task.status === "postprocessing")) {
        const status = event.payload.status;
        if (status === "postprocessing") {
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
          if (event.payload.downloaded) task.downloaded = event.payload.downloaded;
          if (event.payload.total) task.total = event.payload.total;
        }
      }
      updateTaskbarProgress();
    });

    await listen<{ id: string; line: string }>("download-log", (event) => {
      const task = tasks.value.find((t) => t.id === event.payload.id);
      if (task) {
        pushTaskLog(task.logs, event.payload.line);
      }
    });

    await listen<{ id: string; outputFile: string; fileSizeBytes?: number }>(
      "download-complete",
      (event) => {
        const task = tasks.value.find((t) => t.id === event.payload.id);
        if (task && (task.status === "downloading" || task.status === "postprocessing")) {
          task.status = "completed";
          task.percent = 100;
          task.speed = "";
          task.eta = "";
          if (event.payload.outputFile) task.outputFile = event.payload.outputFile;
          if (typeof event.payload.fileSizeBytes === "number" && event.payload.fileSizeBytes > 0) {
            task.fileSizeBytes = event.payload.fileSizeBytes;
            task.total = formatFileSize(event.payload.fileSizeBytes);
          }
          notify(
            i18n.global.t("downloads.notifyComplete"),
            task.title || i18n.global.t("downloads.notifyCompleteBody"),
          );
          // Preserve the bounded log buffer for diagnostics after completion.
          void updateTask(task);
        }
        updateTaskbarProgress();
        void tryStartNext();
      },
    );

    await listen<{ id: string; error: string }>("download-error", (event) => {
      const task = tasks.value.find((t) => t.id === event.payload.id);
      if (task && task.status !== "cancelled") {
        task.status = "error";
        task.error = event.payload.error;
        task.speed = "";
        task.eta = "";
        // 错误诊断依赖日志，保留裁剪后的 logs 并落库
        void updateTask(task);
      }
      updateTaskbarProgress();
      void tryStartNext();
    });
  };

  const ready = Promise.all([loadTasks(), setupListeners()]);
  void ready.then(() => tryStartNext());
  watch(
    () => settingStore.maxConcurrent,
    (max, previous) => {
      if (max > previous) void tryStartNext();
    },
  );

  /** 同一任务的写入串行执行，避免较早的 queued 覆盖稍后的 cancelled。 */
  const persistTask = async (task: DownloadTask): Promise<boolean> => {
    const id = task.id;
    const previous = pendingWrites.get(id);
    const record = {
      ...task,
      logs: [...task.logs],
      params: { ...task.params, subtitles: [...task.params.subtitles] },
    };
    const writing = (async () => {
      if (previous) await previous;
      try {
        await invoke("db_upsert_task", { task: record });
        return true;
      } catch (error) {
        console.error("写入任务至数据库失败:", error);
        return false;
      }
    })();
    pendingWrites.set(id, writing);
    try {
      return await writing;
    } finally {
      if (pendingWrites.get(id) === writing) pendingWrites.delete(id);
    }
  };

  /**
   * 添加任务（或替换解析占位），落库成功后统一交给队列调度。
   *
   * 必须 await：确保行落库后再启动子进程，否则后端的状态更新会写到空行上。
   *
   * @param task 新创建的下载任务对象
   */
  const addTask = async (
    task: DownloadTask,
    preparingTaskId?: string,
  ): Promise<"started" | "queued" | "failed" | "preparing"> => {
    await ready;
    let target: DownloadTask;
    if (preparingTaskId) {
      const preparing = tasks.value.find((candidate) => candidate.id === preparingTaskId);
      if (!preparing || preparing.status !== "preparing") return "failed";
      target = preparing;
    } else {
      if (tasks.value.some((candidate) => candidate.id === task.id)) return "failed";
      tasks.value.unshift(task);
      target = tasks.value[0];
    }

    pendingTaskIds.add(task.id);
    Object.assign(target, task);
    if (target.status !== "preparing") target.status = "queued";
    try {
      if (preparingTaskId) {
        await pendingWrites.get(preparingTaskId);
        try {
          await invoke("db_delete_task", { id: preparingTaskId });
        } catch {
          // 旧占位行可能已不存在。
        }
      }
      if (!tasks.value.includes(target)) return "failed";
      if (!(await persistTask(target))) {
        if (target.status === "queued" || target.status === "preparing") {
          target.status = "error";
          target.error = i18n.global.t("downloads.startFailed");
        }
        return "failed";
      }
    } finally {
      pendingTaskIds.delete(task.id);
    }

    await tryStartNext();
    if (target.status === "preparing") return "preparing";
    if (target.status === "queued") return "queued";
    if (target.status === "error" || target.status === "cancelled") return "failed";
    return tasks.value.includes(target) ? "started" : "failed";
  };

  /**
   * 更新已有任务的元数据与状态，并同步至 SQLite 数据库
   *
   * @param task 待更新的下载任务对象
   */
  const updateTask = async (task: DownloadTask): Promise<void> => {
    const idx = tasks.value.findIndex((t) => t.id === task.id);
    if (idx !== -1) {
      Object.assign(tasks.value[idx], task);
    }
    await persistTask(task);
  };

  /**
   * 取消指定的下载任务（由后端原生终止进程并直接落库为已取消状态）
   *
   * @param id 任务 ID
   * @returns 取消流程 Promise
   */
  const cancelTask = (id: string): Promise<void> => {
    const existing = cancellations.get(id);
    if (existing) return existing;
    const task = tasks.value.find((t) => t.id === id);
    if (
      !task ||
      (task.status !== "queued" &&
        task.status !== "preparing" &&
        task.status !== "downloading" &&
        task.status !== "postprocessing")
    ) {
      return Promise.resolve();
    }

    const hadNoProcess = task.status === "queued" || task.status === "preparing";
    if (!hadNoProcess) cancellingIds.add(id);
    task.status = "cancelled";
    task.speed = "";
    task.eta = "";

    const cancelling = (async () => {
      if (!hadNoProcess) {
        // start_download can still be checking FFmpeg before registering its process.
        await startingTasks.get(id)?.catch(() => {});
        try {
          await invoke("cancel_download", { id, deleteFiles: false });
        } catch {
          // 进程可能已经提前退出。
        }
      }
      await updateTask(task);
      updateTaskbarProgress();
    })().finally(() => {
      cancellations.delete(id);
      cancellingIds.delete(id);
      void tryStartNext();
    });
    cancellations.set(id, cancelling);
    return cancelling;
  };

  /**
   * 重新下载失败或已取消的任务（原位重试，更新原有槽位）
   *
   * @param id 待重试的任务 ID
   * @param editedParams 本次重试的完整参数快照，不修改全局偏好
   * @returns 重试流程 Promise
   */
  const retryTask = async (id: string, editedParams?: DownloadTaskParams): Promise<boolean> => {
    const task = tasks.value.find((taskItem) => taskItem.id === id);
    if (
      !task ||
      (task.status !== "error" && task.status !== "cancelled") ||
      retryingTasks.has(task) ||
      pendingTaskIds.has(id) ||
      cancellations.has(id)
    ) {
      return false;
    }
    retryingTasks.add(task);

    const newId = `dl_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`;
    const oldId = task.id;
    pendingTaskIds.add(newId);
    task.id = newId;
    const params = normalizeTaskParams(editedParams ?? task.params);
    task.params = { ...params, subtitles: [...params.subtitles] };
    if (editedParams) {
      task.formatLabel = i18n.global.t(
        params.downloadMode === "audio"
          ? "detail.audioOnly"
          : params.downloadMode === "video"
            ? "detail.videoOnly"
            : "detail.defaultQuality",
      );
    }
    task.status = "queued";
    task.percent = 0;
    task.speed = "";
    task.eta = "";
    task.downloaded = "";
    task.total = "";
    task.logs = [];
    task.error = undefined;
    task.outputFile = undefined;
    task.fileSizeBytes = undefined;

    try {
      // Drain older writes before deleting the former ID, so they cannot recreate it.
      await pendingWrites.get(oldId);
      try {
        await invoke("db_delete_task", { id: oldId });
      } catch {
        // 旧行可能已不存在，继续。
      }
      if (!tasks.value.includes(task)) return false;
      if (!(await persistTask(task)) && task.status === "queued") {
        task.status = "error";
        task.error = i18n.global.t("downloads.startFailed");
      }
    } finally {
      pendingTaskIds.delete(newId);
      retryingTasks.delete(task);
    }
    await tryStartNext();
    const current = tasks.value.find((candidate) => candidate.id === newId);
    return Boolean(current && current.status !== "error" && current.status !== "cancelled");
  };

  /**
   * 针对已完成任务的「重新下载」：
   * 绝不覆盖原有的已完成历史记录，克隆配置生成新任务压入下载列表
   *
   * @param id 目标历史任务 ID
   * @returns 流程 Promise
   */
  const reDownloadTask = async (id: string): Promise<void> => {
    const existingTask = tasks.value.find((taskItem) => taskItem.id === id);
    if (!existingTask || existingTask.status !== "completed") return;

    const newTaskId = `dl_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`;
    const fullParams = normalizeTaskParams(existingTask.params);

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

    await addTask(newTask);
  };

  /**
   * 一键重试所有处于失败或已取消状态的中断任务
   *
   * @returns 流程 Promise
   */
  const retryAllInterrupted = async (): Promise<void> => {
    const targetTasks = tasks.value.filter(
      (task) => task.status === "error" || task.status === "cancelled",
    );
    for (const targetTask of targetTasks) {
      await retryTask(targetTask.id);
    }
  };

  /**
   * 从列表中移除指定任务并从 SQLite 删除
   *
   * @param id 任务 ID
   */
  const removeTask = async (id: string): Promise<void> => {
    await cancelTask(id);
    const idx = tasks.value.findIndex((taskItem) => taskItem.id === id);
    if (idx !== -1) tasks.value.splice(idx, 1);
    await pendingWrites.get(id);
    try {
      await invoke("db_delete_task", { id });
    } catch {
      // 记录已从内存移除，落库失败仅记录
    }
  };

  const clearTasks = async (
    statuses: DownloadTask["status"][],
    cleanResiduals: boolean,
  ): Promise<void> => {
    const targets = tasks.value.filter((task) => statuses.includes(task.status));
    const ids = targets.map((task) => task.id);
    const cancelling = ids.map((id) => cancelTask(id));
    tasks.value = tasks.value.filter((task) => !ids.includes(task.id));
    // Pending cancellation and persistence must finish before deletion, or an
    // older upsert can recreate a row the user has just cleared.
    await Promise.all(cancelling);
    await Promise.all(ids.map((id) => pendingWrites.get(id)));
    if (ids.length > 0) {
      await invoke("db_delete_tasks", { ids }).catch(() => {});
      if (cleanResiduals) await cleanMultipleTasksResiduals(targets);
    }
  };

  /**
   * 仅清空所有已成功完成的任务
   */
  const clearCompleted = (): Promise<void> => clearTasks(["completed"], false);

  /**
   * 仅清空已取消的任务（同时清理磁盘残留临时未合并文件）
   *
   * @returns 清理流程 Promise
   */
  const clearCancelled = (): Promise<void> => clearTasks(["cancelled"], true);

  /**
   * 仅清空下载失败的任务（同时清理磁盘残留临时未合并文件）
   *
   * @returns 清理流程 Promise
   */
  const clearFailed = (): Promise<void> => clearTasks(["error"], true);

  /**
   * 仅清空所有失败与已取消的任务（同时清理磁盘残留临时文件）
   *
   * @returns 清理流程 Promise
   */
  const clearInterrupted = (): Promise<void> => clearTasks(["error", "cancelled"], true);

  return {
    tasks,
    loaded,
    activeCount,
    loadTasks,
    addTask,
    updateTask,
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
