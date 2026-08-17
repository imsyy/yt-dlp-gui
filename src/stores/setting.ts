import { defineStore } from "pinia";
import { setI18nLocale, resolveLocale } from "@/locales";
import { DEFAULT_OUTPUT_TEMPLATE } from "@/utils/output-template";
import type { HomeDownloadBehavior, HomeMode } from "@/types";

export const useSettingStore = defineStore(
  "setting",
  () => {
    /** 界面语言 */
    const locale = ref(resolveLocale(""));

    watch(locale, (val) => {
      setI18nLocale(val);
    });

    /** 主题模式 */
    const themeMode = ref<"auto" | "light" | "dark">("auto");

    /** 首页输入模式与解析后的处理方式 */
    const homeMode = ref<HomeMode>("standard");
    const homeDownloadBehavior = ref<HomeDownloadBehavior>("pending");

    /** 快速下载默认参数 */
    const quickDownloadMode = ref<"default" | "video" | "audio">("default");
    const quickMaxHeight = ref(1080);
    const quickEmbedThumbnail = ref(false);
    const quickEmbedMetadata = ref(false);
    const quickEmbedChapters = ref(false);
    const quickSponsorblockRemove = ref(false);
    const quickNoMerge = ref(false);
    const quickRecodeFormat = ref("");
    const quickLimitRate = ref("");
    const quickFfmpegArgs = ref("");

    /** 下载目录 */
    const downloadDir = ref("");

    /** Cookie 模式 */
    const cookieMode = ref<"none" | "text" | "file" | "browser">("none");

    /** Cookie 文本内容（Netscape 格式） */
    const cookieText = ref("");

    /** Cookie 文件路径 */
    const cookieFile = ref("");

    /** 从浏览器读取 Cookie 的浏览器名称 */
    const cookieBrowser = ref("chrome");

    /** 代理地址 */
    const proxy = ref("");

    /** 文件名输出模板 */
    const outputTemplate = ref(DEFAULT_OUTPUT_TEMPLATE);

    /** 文件名静态前缀/后缀（后缀插入扩展名前） */
    const filenamePrefix = ref("");
    const filenameSuffix = ref("");

    /** 并发分片数，0 = 不启用 */
    const concurrentFragments = ref(0);

    /** 文件已存在时不覆盖 */
    const noOverwrites = ref(false);

    /** 新下载任务默认使用的 FFmpeg 后处理参数 */
    const defaultFfmpegArgs = ref("");

    /** 最大同时下载数，0 = 不限制 */
    const maxConcurrentDownloads = ref(0);

    /** 下载完成通知模式 */
    const notifyMode = ref<"none" | "app" | "system" | "all">("system");

    /** 关闭窗口时最小化到托盘 */
    const closeToTray = ref(true);

    /** 启动时自动检查更新 */
    const autoCheckUpdate = ref(true);

    /** 每个外部工具独立选择应用管理版本或系统 PATH 版本 */
    const ytdlpSource = ref<"managed" | "system">("managed");
    const denoSource = ref<"managed" | "system">("managed");
    const ffmpegSource = ref<"managed" | "system">("system");

    /** YouTube PO Token（用于绕过 403 / 限流） */
    const youtubePoToken = ref("");

    /** YouTube visitor_data（与 PO Token 配套） */
    const youtubeVisitorData = ref("");

    /** 在任务栏显示下载进度 */
    const showTaskbarProgress = ref(true);

    return {
      locale,
      themeMode,
      homeMode,
      homeDownloadBehavior,
      quickDownloadMode,
      quickMaxHeight,
      quickEmbedThumbnail,
      quickEmbedMetadata,
      quickEmbedChapters,
      quickSponsorblockRemove,
      quickNoMerge,
      quickRecodeFormat,
      quickLimitRate,
      quickFfmpegArgs,
      downloadDir,
      cookieMode,
      cookieText,
      cookieFile,
      cookieBrowser,
      proxy,
      outputTemplate,
      filenamePrefix,
      filenameSuffix,
      concurrentFragments,
      noOverwrites,
      defaultFfmpegArgs,
      maxConcurrentDownloads,
      notifyMode,
      closeToTray,
      autoCheckUpdate,
      ytdlpSource,
      denoSource,
      ffmpegSource,
      youtubePoToken,
      youtubeVisitorData,
      showTaskbarProgress,
    };
  },
  {
    persist: true,
  },
);
