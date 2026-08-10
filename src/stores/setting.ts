import { defineStore } from "pinia";
import { setI18nLocale, resolveLocale } from "@/locales";

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

    /** 下载目录 */
    const downloadDir = ref("");

    /** FFmpeg 所在目录，空值表示使用系统 PATH 中的 ffmpeg */
    const ffmpegDir = ref("");

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
    const outputTemplate = ref("%(title).200s [%(id)s].%(ext)s");

    /** 嵌入字幕（额外选项默认值，记住上次选择） */
    const embedSubs = ref(false);

    /** 嵌入缩略图（额外选项默认值） */
    const embedThumbnail = ref(false);

    /** 嵌入元数据（额外选项默认值） */
    const embedMetadata = ref(false);

    /** 嵌入章节（额外选项默认值） */
    const embedChapters = ref(false);

    /** 跳过赞助片段 SponsorBlock（额外选项默认值） */
    const sponsorblockRemove = ref(false);

    /** 提取音频模式（额外选项默认值） */
    const extractAudio = ref(false);

    /** 音频转换格式（额外选项默认值） */
    const audioConvertFormat = ref("");

    /** 不合并音视频流（额外选项默认值） */
    const noMerge = ref(false);

    /** 视频转换格式（额外选项默认值） */
    const recodeFormat = ref("");

    /** 下载限速（额外选项默认值） */
    const limitRate = ref("");

    /** 自定义 FFmpeg 参数（额外选项默认值） */
    const ffmpegArgs = ref("");

    /** 并发分片数，0 = 不启用 */
    const concurrentFragments = ref(0);

    /** 文件已存在时不覆盖 */
    const noOverwrites = ref(false);

    /** 清空已完成任务前是否二次确认 */
    const confirmClearCompleted = ref(true);

    /** 最大同时下载数，0 = 不限制 */
    const maxConcurrentDownloads = ref(0);

    /** 下载完成通知模式 */
    const notifyMode = ref<"none" | "app" | "system" | "all">("system");

    /** 关闭窗口时最小化到托盘 */
    const closeToTray = ref(true);

    /** 启动时自动检查更新 */
    const autoCheckUpdate = ref(true);

    /** 二进制路径解析模式（默认仅应用，保证「检测更新」始终对实际使用的副本生效） */
    const binaryPathResolveMode = ref<"system-preferred" | "app-only">("app-only");

    /** YouTube PO Token（用于绕过 403 / 限流） */
    const youtubePoToken = ref("");

    /** YouTube visitor_data（与 PO Token 配套） */
    const youtubeVisitorData = ref("");

    /** 在任务栏显示下载进度 */
    const showTaskbarProgress = ref(true);

    return {
      locale,
      themeMode,
      downloadDir,
      ffmpegDir,
      cookieMode,
      cookieText,
      cookieFile,
      cookieBrowser,
      proxy,
      outputTemplate,
      embedSubs,
      embedThumbnail,
      embedMetadata,
      embedChapters,
      sponsorblockRemove,
      extractAudio,
      audioConvertFormat,
      noMerge,
      recodeFormat,
      limitRate,
      ffmpegArgs,
      concurrentFragments,
      noOverwrites,
      confirmClearCompleted,
      maxConcurrentDownloads,
      notifyMode,
      closeToTray,
      autoCheckUpdate,
      binaryPathResolveMode,
      youtubePoToken,
      youtubeVisitorData,
      showTaskbarProgress,
    };
  },
  {
    persist: true,
  },
);
