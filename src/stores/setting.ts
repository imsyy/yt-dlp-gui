import { defineStore } from "pinia";
import { setI18nLocale, resolveLocale } from "@/locales";

export const useSettingStore = defineStore("setting", () => {
  /** 界面语言 */
  const locale = ref(resolveLocale(""));

  watch(locale, (val) => {
    setI18nLocale(val);
  });

  /** 主题模式 */
  const themeMode = ref<"auto" | "light" | "dark">("auto");

  /** 下载目录 */
  const downloadDir = ref("");

  /** Cookie 模式 */
  const cookieMode = ref<"none" | "text" | "file">("none");

  /** Cookie 文本内容（Netscape 格式） */
  const cookieText = ref("");

  /** Cookie 文件路径 */
  const cookieFile = ref("");

  /** 代理地址 */
  const proxy = ref("");

  /** 文件名输出模板 */
  const outputTemplate = ref("%(title).200s [%(id)s].%(ext)s");

  /** 并发分片数，0 = 不启用 */
  const concurrentFragments = ref(0);

  /** 文件已存在时不覆盖 */
  const noOverwrites = ref(false);

  /** 最大同时下载数，0 = 不限制 */
  const maxConcurrentDownloads = ref(0);

  /** 下载完成通知模式 */
  const notifyMode = ref<"none" | "app" | "system" | "all">("system");

  /** 关闭窗口时最小化到托盘 */
  const closeToTray = ref(true);

  return {
    locale,
    themeMode,
    downloadDir,
    cookieMode,
    cookieText,
    cookieFile,
    proxy,
    outputTemplate,
    concurrentFragments,
    noOverwrites,
    maxConcurrentDownloads,
    notifyMode,
    closeToTray,
  };
}, {
  persist: true,
});
