import { defineStore } from "pinia";

export const useSettingStore = defineStore("setting", () => {
  /** 主题模式: auto 跟随系统, light 亮色, dark 暗色 */
  const themeMode = ref<"auto" | "light" | "dark">("auto");

  /** 默认下载目录 */
  const downloadDir = ref("");

  /** Cookie 模式: none 不使用, text 手动输入, file 文件读取 */
  const cookieMode = ref<"none" | "text" | "file">("none");

  /** Cookie 文本内容（Netscape 格式） */
  const cookieText = ref("");

  /** Cookie 文件路径 */
  const cookieFile = ref("");

  /** 代理地址，如 http://127.0.0.1:7890 或 socks5://... */
  const proxy = ref("");

  /** 文件名模板，使用 yt-dlp output template 语法 */
  const outputTemplate = ref("%(title).200s [%(id)s].%(ext)s");

  /** 并发分片数（--concurrent-fragments），0 表示不启用 */
  const concurrentFragments = ref(0);

  /** 文件已存在时不覆盖 */
  const noOverwrites = ref(false);

  /** 最大同时下载任务数，0 表示不限制 */
  const maxConcurrentDownloads = ref(0);

  /** 下载完成通知模式: none 不通知, app 应用内, system 系统通知, all 全部 */
  const notifyMode = ref<"none" | "app" | "system" | "all">("system");

  return {
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
  };
}, {
  persist: true,
});
