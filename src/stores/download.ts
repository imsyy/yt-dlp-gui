import { defineStore } from "pinia";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow, ProgressBarStatus } from "@tauri-apps/api/window";
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";
import type { DownloadTask } from "@/types";
import { useSettingStore } from "@/stores/setting";
import { formatFileSize } from "@/utils/format";
import { cleanMultipleTasksResiduals } from "@/utils/taskFiles";
import { migrateLegacyTasks, normalizeTaskParams } from "@/utils/migration";
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
  const tasks = ref<DownloadTask[]>([]);
  const loaded = ref(false);
  let listenersSetup = false;

  /** 当前占用下载进程的任务数（后处理尚未释放进程，也计入并发槽位） */
  const activeCount = computed(
    () =>
      tasks.value.filter(
        (task) => task.status === "downloading" || task.status === "postprocessing",
      ).length,
  );

  /**
   * 尝试启动排队中的下一个就绪任务
   *
   * @returns 启动流程 Promise
   */
  const tryStartNext = async (): Promise<void> => {
    const settingStore = useSettingStore();
    const max = settingStore.maxConcurrentDownloads;
    if (max > 0 && activeCount.value >= max) return;

    const next = tasks.value.find((taskItem) => taskItem.status === "queued");
    if (!next) return;

    const fullParams = normalizeTaskParams(next.params);
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

  /**
   * 判断当前是否允许立即开启新下载（未超最大并发数）
   *
   * @returns 是否能够立即开始下载
   */
  const canStartNow = (): boolean => {
    const settingStore = useSettingStore();
    const max = settingStore.maxConcurrentDownloads;
    return max <= 0 || activeCount.value < max;
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
   * 从后端 SQLite 加载任务列表并自动执行必要的清理与迁移
   *
   * 1. 优先触发老版本 IndexedDB 数据的静默平滑迁移；
   * 2. 从 SQLite 读取全部任务列表（后端冷启动时已将异常退出的进行中任务归一化为错误中断态）；
   * 3. 异步探测已完成任务的物理输出文件是否存在，若已被外部删除则同步清除失效记录。
   *
   * @returns 加载任务完成 Promise
   */
  const loadTasks = async (): Promise<void> => {
    try {
      // 优先执行老版本 IndexedDB 数据的静默平滑迁移
      await migrateLegacyTasks();

      const dbTasks = await invoke<DownloadTask[]>("db_get_tasks");
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

  /**
   * 添加新的下载任务到列表顶部，并异步写入 SQLite 持久化
   *
   * @param task 新创建的下载任务对象
   */
  const addTask = (task: DownloadTask): void => {
    tasks.value.unshift(task);
    invoke("db_upsert_task", { task }).catch((err) => {
      console.error("写入任务至数据库失败:", err);
    });
  };

  /**
   * 更新已有任务的元数据与状态，并异步同步至 SQLite 数据库
   *
   * @param task 待更新的下载任务对象
   */
  const updateTask = (task: DownloadTask): void => {
    const idx = tasks.value.findIndex((t) => t.id === task.id);
    if (idx !== -1) {
      Object.assign(tasks.value[idx], task);
    }
    invoke("db_upsert_task", { task }).catch((err) => {
      console.error("更新任务至数据库失败:", err);
    });
  };

  /**
   * 取消指定的下载任务（由后端原生终止进程并直接落库为已取消状态）
   *
   * @param id 任务 ID
   * @returns 取消流程 Promise
   */
  const cancelTask = async (id: string): Promise<void> => {
    const task = tasks.value.find((t) => t.id === id);
    if (!task) return;

    const hadNoProcess = task.status === "queued" || task.status === "preparing";
    task.status = "cancelled";

    if (!hadNoProcess) {
      try {
        await invoke("cancel_download", { id, deleteFiles: false });
      } catch {
        // 进程可能已经提前退出
      }
    } else {
      // 未真正启动子进程的就绪态任务，前端主动补记一条状态更新
      invoke("db_upsert_task", { task }).catch(() => {});
    }

    updateTaskbarProgress();
    tryStartNext();
  };

  /**
   * 重新下载失败或已取消的任务（原位重试，更新原有槽位）
   *
   * @param id 待重试的任务 ID
   * @returns 重试流程 Promise
   */
  const retryTask = async (id: string): Promise<void> => {
    const task = tasks.value.find((taskItem) => taskItem.id === id);
    if (!task) return;

    const newId = `dl_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`;
    const fullParams = normalizeTaskParams(task.params);
    const oldId = task.id;

    // 先从 SQLite 移除旧任务记录
    invoke("db_delete_task", { id: oldId }).catch(() => {});

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
      invoke("db_upsert_task", { task }).catch(() => {});
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
      invoke("db_upsert_task", { task }).catch(() => {});
    }
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
    if (!existingTask) return;

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

    tasks.value.unshift(newTask);
    invoke("db_upsert_task", { task: newTask }).catch(() => {});

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
  const removeTask = (id: string): void => {
    const idx = tasks.value.findIndex((taskItem) => taskItem.id === id);
    if (idx !== -1) tasks.value.splice(idx, 1);
    invoke("db_delete_task", { id }).catch(() => {});
  };

  /**
   * 仅清空所有已成功完成的任务
   */
  const clearCompleted = (): void => {
    tasks.value = tasks.value.filter((task) => task.status !== "completed");
    invoke("db_clear_completed_tasks").catch(() => {});
  };

  /**
   * 仅清空已取消的任务（同时清理磁盘残留临时未合并文件）
   *
   * @returns 清理流程 Promise
   */
  const clearCancelled = async (): Promise<void> => {
    const cancelledTasks = tasks.value.filter((task) => task.status === "cancelled");
    const ids = cancelledTasks.map((t) => t.id);
    tasks.value = tasks.value.filter((task) => task.status !== "cancelled");
    if (ids.length > 0) {
      invoke("db_delete_tasks", { ids }).catch(() => {});
      await cleanMultipleTasksResiduals(cancelledTasks);
    }
  };

  /**
   * 仅清空下载失败的任务（同时清理磁盘残留临时未合并文件）
   *
   * @returns 清理流程 Promise
   */
  const clearFailed = async (): Promise<void> => {
    const failedTasks = tasks.value.filter((task) => task.status === "error");
    const ids = failedTasks.map((t) => t.id);
    tasks.value = tasks.value.filter((task) => task.status !== "error");
    if (ids.length > 0) {
      invoke("db_delete_tasks", { ids }).catch(() => {});
      await cleanMultipleTasksResiduals(failedTasks);
    }
  };

  /**
   * 仅清空所有失败与已取消的任务（同时清理磁盘残留临时文件）
   *
   * @returns 清理流程 Promise
   */
  const clearInterrupted = async (): Promise<void> => {
    const interruptedTasks = tasks.value.filter(
      (task) => task.status === "error" || task.status === "cancelled",
    );
    const ids = interruptedTasks.map((t) => t.id);
    tasks.value = tasks.value.filter(
      (task) => task.status !== "error" && task.status !== "cancelled",
    );
    if (ids.length > 0) {
      invoke("db_delete_tasks", { ids }).catch(() => {});
      await cleanMultipleTasksResiduals(interruptedTasks);
    }
  };

  return {
    tasks,
    loaded,
    activeCount,
    canStartNow,
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
