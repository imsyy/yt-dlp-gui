import { invoke } from "@tauri-apps/api/core";
import { check as checkForAppUpdate } from "@tauri-apps/plugin-updater";
import { useSettingStore } from "@/stores/setting";
import { useStatusStore } from "@/stores/status";

/**
 * 封装应用启动引导与底层服务同步逻辑的 Composable：
 * 1. CLI 工具源设置同步（managed / system / custom）
 * 2. yt-dlp 分支通道同步（stable / nightly / master）
 * 3. 下载目录自动检测（首次启动或目录为空时）
 * 4. 应用自动更新检查
 *
 * @returns 包含启动方法与各个同步操作的对象
 */
export const useAppBootstrap = () => {
  const settingStore = useSettingStore();
  const statusStore = useStatusStore();

  /**
   * 将前端持久化的 CLI 工具源同步至 Rust 后端
   *
   * @returns Promise<unknown>
   */
  const applyToolSources = (): Promise<unknown> =>
    invoke("set_tool_sources", {
      ytdlp: settingStore.ytdlpSource,
      deno: settingStore.denoSource,
      bun: settingStore.bunSource,
      ffmpeg: settingStore.ffmpegSource,
    });

  /**
   * 将前端配置的 yt-dlp 分支通道同步至 Rust 后端
   *
   * @returns Promise<unknown>
   */
  const applyYtdlpChannel = (): Promise<unknown> =>
    invoke("set_ytdlp_channel", { channel: settingStore.ytdlpChannel }).catch(() => {});

  const applyJsRuntime = (): Promise<unknown> =>
    invoke("set_js_runtime", { runtime: settingStore.jsRuntime }).catch(() => {});

  /**
   * 检查应用是否有新版本可用，有更新时自动弹出模态框
   *
   * @returns Promise<void>
   */
  const checkAppUpdate = async (): Promise<void> => {
    try {
      const updateResult = await checkForAppUpdate();
      if (updateResult) {
        statusStore.updateVersion = updateResult.version;
        statusStore.updateNotes = updateResult.body || "";
        statusStore.showUpdateModal = true;
      }
    } catch {
      // 静默失败，不打扰用户正常使用
    }
  };

  /**
   * 自动检测并设置默认下载目录（仅在首次启动或目录为空时）
   *
   * @returns Promise<void>
   */
  const autoDetectDownloadDir = async (): Promise<void> => {
    if (settingStore.downloadDir) return;
    try {
      const defaultDir = await invoke<string>("get_default_download_dir");
      settingStore.downloadDir = defaultDir;
    } catch {
      // 静默失败，用户可以手动设置
    }
  };

  /**
   * 初始化下载任务列表和事件监听
   *
   * @returns Promise<void>
   */
  const initializeDownloadStore = async (): Promise<void> => {
    const { useDownloadStore } = await import("@/stores/download");
    const downloadStore = useDownloadStore();
    await downloadStore.loadTasks();
    await downloadStore.setupListeners();
  };

  /**
   * 执行完整的应用启动引导任务
   *
   * @returns Promise<void>
   */
  const bootstrap = async (): Promise<void> => {
    watch(
      () => [
        settingStore.ytdlpSource,
        settingStore.denoSource,
        settingStore.bunSource,
        settingStore.ffmpegSource,
      ],
      () => applyToolSources(),
    );

    watch(
      () => settingStore.ytdlpChannel,
      () => applyYtdlpChannel(),
    );

    watch(
      () => settingStore.jsRuntime,
      () => applyJsRuntime(),
    );

    await applyToolSources();
    await applyYtdlpChannel();
    await applyJsRuntime();
    await autoDetectDownloadDir();
    await initializeDownloadStore();

    if (settingStore.autoCheckUpdate) {
      void checkAppUpdate();
    }
  };

  return {
    bootstrap,
    applyToolSources,
    applyYtdlpChannel,
    applyJsRuntime,
    checkAppUpdate,
    autoDetectDownloadDir,
    initializeDownloadStore,
  };
};
