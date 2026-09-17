import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { onOpenUrl, getCurrent as getCurrentDeepLink } from "@tauri-apps/plugin-deep-link";
import { useSettingStore } from "@/stores/setting";
import { normalizeDeepLinkVideoUrl } from "@/utils/url";
import type { BrowserExtensionImport, CliOpenRequest, HomeMode } from "@/types";

/**
 * 集中管理应用外部导入能力的 Composable：
 * 1. 浏览器扩展导入（Local HTTP Bridge）
 * 2. 深度链接唤醒（Deep Link: ytdlp-gui://）
 * 3. 命令行参数导入（CLI Arguments）
 *
 * @returns 包含监听器初始化及各导入处理方法的对象
 */
export const useExternalImports = () => {
  const router = useRouter();
  const settingStore = useSettingStore();

  let lastDeepLinkUrl = "";
  let lastDeepLinkTimestamp = 0;

  /**
   * 处理深链接唤醒 URL（如 ytdlp-gui://download?url=...&mode=batch）
   *
   * @param rawDeepLinkUrl 操作系统传入的原始协议 URL 字符串
   */
  const handleDeepLink = (rawDeepLinkUrl: string): void => {
    const currentTimestamp = Date.now();
    if (rawDeepLinkUrl === lastDeepLinkUrl && currentTimestamp - lastDeepLinkTimestamp < 1500) {
      return;
    }
    lastDeepLinkUrl = rawDeepLinkUrl;
    lastDeepLinkTimestamp = currentTimestamp;

    try {
      const parsedUrl = new URL(rawDeepLinkUrl);
      if (parsedUrl.host !== "download") return;

      const videoUrl = parsedUrl.searchParams.get("url");
      if (!videoUrl) return;

      const targetMode = parsedUrl.searchParams.get("mode") as HomeMode | null;
      const queryParams: Record<string, string> = {
        url: normalizeDeepLinkVideoUrl(videoUrl),
        _t: String(Date.now()),
      };

      if (targetMode === "standard" || targetMode === "batch") {
        queryParams.mode = targetMode;
      }

      router.push({ name: "home", query: queryParams });
    } catch {
      // 忽略非法格式的深链接
    }
  };

  /**
   * 处理命令行传入的启动请求参数（支持冷启动与单实例二次启动）
   *
   * @param request 解析出的命令行参数结构
   */
  const handleCliOpenRequest = (request: CliOpenRequest): void => {
    if (request.cookieFile) {
      settingStore.cookieFile = request.cookieFile;
      settingStore.cookieMode = "file";
    }
    if (request.downloadDir) {
      settingStore.downloadDir = request.downloadDir;
    }
    if (request.url) {
      router.push({
        name: "home",
        query: { url: request.url, _t: String(Date.now()) },
      });
    }
  };

  /**
   * 处理从浏览器扩展通过本地 HTTP 桥接发送过来的数据（单链接或批量链接，附带 Cookie）
   *
   * @param importedData 浏览器插件导入的数据载荷
   */
  const handleBrowserExtensionImport = (importedData: BrowserExtensionImport): void => {
    console.log("[YDL GUI] browser extension import received:", importedData);
    if (importedData.cookieFile) {
      settingStore.cookieFile = importedData.cookieFile;
      settingStore.cookieMode = "file";
    }

    const rawUrlList =
      importedData.urls && importedData.urls.length > 0
        ? importedData.urls
        : importedData.url
          ? [importedData.url]
          : [];

    const normalizedUrls = rawUrlList.map(normalizeDeepLinkVideoUrl).filter(Boolean);

    const queryParams: Record<string, string> = {
      _t: String(Date.now()),
    };

    if (normalizedUrls.length > 0) {
      queryParams.url = normalizedUrls[0];
      if (normalizedUrls.length > 1) {
        queryParams.urls = JSON.stringify(normalizedUrls);
      }
    }

    if (importedData.mode) {
      queryParams.mode = importedData.mode;
    } else if (normalizedUrls.length > 1) {
      queryParams.mode = "batch";
    }

    router.push({ name: "home", query: queryParams });
  };

  /**
   * 消费 Rust 后台本地桥暂存的浏览器扩展导入数据队列
   *
   * @returns Promise<void>
   */
  const consumeBrowserExtensionImports = async (): Promise<void> => {
    try {
      const pendingImports = await invoke<BrowserExtensionImport[]>(
        "take_browser_extension_imports",
      );
      for (const importItem of pendingImports) {
        handleBrowserExtensionImport(importItem);
      }
    } catch (error) {
      console.error("[YDL GUI] failed to consume browser extension imports:", error);
    }
  };

  /**
   * 初始化所有外部导入事件监听器（包括协议、CLI、扩展桥接）
   *
   * @returns Promise<void>
   */
  const setupExternalImportListeners = async (): Promise<void> => {
    // 1. 监听浏览器插件导入就绪通知并消费
    await listen("browser-extension-import-ready", () => {
      void consumeBrowserExtensionImports();
    });
    await consumeBrowserExtensionImports();

    // 2. 监听多实例第二进程转发的命令行参数
    await listen<CliOpenRequest>("cli-open-request", (event) => {
      handleCliOpenRequest(event.payload);
    });

    // 3. 读取冷启动初始 CLI 参数
    try {
      const initialCliRequest = await invoke<CliOpenRequest | null>("take_cli_open_request");
      if (initialCliRequest) {
        handleCliOpenRequest(initialCliRequest);
      }
    } catch (error) {
      console.error("[YDL GUI] failed to take initial CLI request:", error);
    }

    // 4. 读取冷启动深度链接
    try {
      const initialDeepLinks = await getCurrentDeepLink();
      if (initialDeepLinks?.length) {
        for (const deepLinkItem of initialDeepLinks) {
          handleDeepLink(deepLinkItem);
        }
      }
    } catch {
      // 插件不可用时静默忽略
    }

    // 5. 应用运行期间接收操作系统深链接
    onOpenUrl((incomingUrls) => {
      for (const incomingUrl of incomingUrls) {
        handleDeepLink(incomingUrl);
      }
    });

    // 6. single-instance 转发的深链接（应用已在运行中）
    await listen<string>("deep-link-url", (event) => {
      handleDeepLink(event.payload);
    });
  };

  return {
    setupExternalImportListeners,
    consumeBrowserExtensionImports,
    handleDeepLink,
    handleCliOpenRequest,
    handleBrowserExtensionImport,
  };
};
