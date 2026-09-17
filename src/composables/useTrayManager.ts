import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { exit } from "@tauri-apps/plugin-process";
import { useI18n } from "vue-i18n";
import { useSettingStore } from "@/stores/setting";
import { useDownloadStore } from "@/stores/download";

/**
 * 封装系统托盘相关逻辑的 Composable：
 * 1. 托盘显示/隐藏控制
 * 2. 托盘菜单多语言文本同步
 * 3. 退出应用确认弹窗
 *
 * @returns 包含托盘初始化与操作方法的对象
 */
export const useTrayManager = () => {
  const { t: translate } = useI18n();
  const settingStore = useSettingStore();
  const downloadStore = useDownloadStore();

  /**
   * 同步托盘右键菜单的语言文本（显示/退出）
   */
  const syncTrayMenu = (): void => {
    invoke("update_tray_menu", {
      showLabel: translate("tray.show"),
      quitLabel: translate("tray.quit"),
    });
  };

  /**
   * 同步托盘图标的系统级可见性
   *
   * @returns Promise<void>
   */
  const syncTrayVisibility = async (): Promise<void> => {
    try {
      await invoke("set_tray_visible", { visible: settingStore.showTrayIcon });
    } catch (error) {
      console.error("[YDL GUI] failed to update tray visibility:", error);
    }
  };

  /**
   * 处理退出应用请求：当存在进行中的下载任务时弹出二次确认弹窗
   */
  const handleQuitRequest = (): void => {
    if (downloadStore.activeCount > 0) {
      window.$dialog.warning({
        title: translate("tray.quitConfirmTitle"),
        content: translate("tray.quitConfirmContent"),
        positiveText: translate("common.cancel"),
        negativeText: translate("tray.quit"),
        onNegativeClick: () => exit(0),
      });
    } else {
      exit(0);
    }
  };

  /**
   * 初始化托盘事件监听与初始状态同步
   *
   * @returns Promise<void>
   */
  const setupTray = async (): Promise<void> => {
    watch(() => settingStore.locale, syncTrayMenu);
    watch(
      () => settingStore.showTrayIcon,
      () => void syncTrayVisibility(),
    );
    await listen("tray-quit-requested", () => handleQuitRequest());
    await syncTrayVisibility();
    syncTrayMenu();
  };

  return {
    setupTray,
    syncTrayMenu,
    syncTrayVisibility,
    handleQuitRequest,
  };
};
