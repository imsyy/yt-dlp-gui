import { defineStore } from "pinia";
import { setI18nLocale, resolveLocale } from "@/locales";
import { DEFAULT_OUTPUT_TEMPLATE } from "@/utils/output-template";
import type { HomeDownloadBehavior, HomeMode, YtdlpChannel } from "@/types";

/** 默认最大同时下载数 */
export const DEFAULT_CONCURRENT_DOWNLOADS = 3;
/** 最大同时下载数下限/上限 */
export const MIN_CONCURRENT_DOWNLOADS = 1;
export const MAX_CONCURRENT_DOWNLOADS = 10;

/**
 * 应用全局偏好配置 Store
 *
 * 负责管理语言、主题、网络代理、下载目录、默认参数等客户端持久化配置（基于 localStorage）。
 */
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
    const quickWriteThumbnail = ref(false);
    const quickWriteDescription = ref(false);
    const quickEmbedMetadata = ref(false);
    const quickEmbedChapters = ref(false);
    const quickSponsorblockRemove = ref(false);
    const quickNoMerge = ref(false);
    const quickRecodeFormat = ref("");
    const quickRemuxFormat = ref("");
    const quickLimitRate = ref("");
    const quickFfmpegArgs = ref("");
    const quickCustomArgs = ref("");

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

    /** 新下载任务默认自定义 yt-dlp 附加参数（如 --sleep-requests 2） */
    const defaultCustomArgs = ref("");

    /** 标准流程新解析任务的默认额外选项 */
    const defaultEmbedSubs = ref(false);
    const defaultEmbedThumbnail = ref(false);
    const defaultWriteThumbnail = ref(false);
    const defaultWriteDescription = ref(false);
    const defaultEmbedMetadata = ref(false);
    const defaultEmbedChapters = ref(false);
    const defaultSponsorblockRemove = ref(false);
    const defaultExtractAudio = ref(false);
    const defaultAudioConvertFormat = ref("");
    const defaultNoMerge = ref(false);
    const defaultRecodeFormat = ref("");
    const defaultRemuxFormat = ref("");
    const defaultLimitRate = ref("");

    /** 最大同时下载数（1~10，默认 3） */
    const maxConcurrentDownloads = ref(3);

    /**
     * 生效的最大同时下载数（带钳制与老数据兼容）
     *
     * 老版本曾用 0 表示"不限制"，升级后统一归一为默认值，避免无上限并发拖垮系统。
     */
    const maxConcurrent = computed(() => {
      const raw = maxConcurrentDownloads.value;
      if (
        !Number.isInteger(raw) ||
        raw < MIN_CONCURRENT_DOWNLOADS ||
        raw > MAX_CONCURRENT_DOWNLOADS
      ) {
        return DEFAULT_CONCURRENT_DOWNLOADS;
      }
      return raw;
    });

    /** 下载完成通知模式 */
    const notifyMode = ref<"none" | "app" | "system" | "all">("system");

    /** 关闭窗口时最小化到托盘 */
    const closeToTray = ref(true);

    /** 显示系统托盘图标 */
    const showTrayIcon = ref(true);

    /** 启动时自动检查更新 */
    const autoCheckUpdate = ref(true);

    /** 每个外部工具独立选择应用管理版本或系统 PATH 版本 */
    const ytdlpSource = ref<"managed" | "system">("managed");
    const denoSource = ref<"managed" | "system">("managed");
    const ffmpegSource = ref<"managed" | "system">("system");

    /** 内置 yt-dlp 的发行通道：stable（稳定版）/ nightly（每日构建）/ master（最新提交构建） */
    const ytdlpChannel = ref<YtdlpChannel>("stable");

    /** 内置 yt-dlp 当前已安装构建所属的通道；跨通道切换（含降级）时以此判断是否需要重新下载 */
    const ytdlpInstalledChannel = ref<YtdlpChannel>("stable");

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
      quickWriteThumbnail,
      quickWriteDescription,
      quickEmbedMetadata,
      quickEmbedChapters,
      quickSponsorblockRemove,
      quickNoMerge,
      quickRecodeFormat,
      quickRemuxFormat,
      quickLimitRate,
      quickFfmpegArgs,
      quickCustomArgs,
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
      defaultCustomArgs,
      defaultEmbedSubs,
      defaultEmbedThumbnail,
      defaultWriteThumbnail,
      defaultWriteDescription,
      defaultEmbedMetadata,
      defaultEmbedChapters,
      defaultSponsorblockRemove,
      defaultExtractAudio,
      defaultAudioConvertFormat,
      defaultNoMerge,
      defaultRecodeFormat,
      defaultRemuxFormat,
      defaultLimitRate,
      maxConcurrentDownloads,
      maxConcurrent,
      notifyMode,
      closeToTray,
      showTrayIcon,
      autoCheckUpdate,
      ytdlpSource,
      denoSource,
      ffmpegSource,
      ytdlpChannel,
      ytdlpInstalledChannel,
      youtubePoToken,
      youtubeVisitorData,
      showTaskbarProgress,
    };
  },
  {
    persist: true,
  },
);
