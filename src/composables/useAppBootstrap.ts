import { watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { check as checkForAppUpdate } from "@tauri-apps/plugin-updater";
import { useSettingStore } from "@/stores/setting";
import { useStatusStore } from "@/stores/status";

/**
 * 封装应用启动引导与底层服务同步逻辑的 Composable：
 * 1. CLI 工具源设置同步（managed / system / custom）
 * 2. yt-dlp 分支通道同步（stable / nightly / master）
 * 3. 应用自动更新检查
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
      ffmpeg: settingStore.ffmpegSource,
    });

  /**
   * 将前端配置的 yt-dlp 分支通道同步至 Rust 后端
   *
   * @returns Promise<unknown>
   */
  const applyYtdlpChannel = (): Promise<unknown> =>
    invoke("set_ytdlp_channel", { channel: settingStore.ytdlpChannel }).catch(() => {});

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
   * 执行完整的应用启动引导任务
   *
   * @returns Promise<void>
   */
  const bootstrap = async (): Promise<void> => {
    watch(
      () => [settingStore.ytdlpSource, settingStore.denoSource, settingStore.ffmpegSource],
      () => applyToolSources(),
    );

    watch(
      () => settingStore.ytdlpChannel,
      () => applyYtdlpChannel(),
    );

    await applyToolSources();
    await applyYtdlpChannel();

    if (settingStore.autoCheckUpdate) {
      void checkAppUpdate();
    }
  };

  return {
    bootstrap,
    applyToolSources,
    applyYtdlpChannel,
    checkAppUpdate,
  };
};
