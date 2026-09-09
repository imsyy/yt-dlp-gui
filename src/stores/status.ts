import { defineStore } from "pinia";
import type { ToolUpdateCheck } from "@/types";

export const useStatusStore = defineStore("status", () => {
  /** Cookie 设置弹窗 */
  const showCookieModal = ref(false);

  /** 应用更新弹窗 */
  const showUpdateModal = ref(false);
  const updateVersion = ref("");
  const updateNotes = ref("");

  /** yt-dlp 未安装弹窗 */
  const showYtdlpSetupModal = ref(false);

  /** Deno 未安装提示弹窗 */
  const showDenoSetupModal = ref(false);

  /** FFmpeg 缺失导致无法合并音视频 */
  const showFfmpegSetupModal = ref(false);

  /**
   * 各外部工具检测到的可用更新（检测更新后写入，安装/更新成功后清除）。
   * ToolManager 与底部状态栏共用：工具行标签切为"有更新"，状态栏图标亮红点。
   */
  const toolUpdates = ref<Record<"yt-dlp" | "deno" | "ffmpeg", ToolUpdateCheck | null>>({
    "yt-dlp": null,
    deno: null,
    ffmpeg: null,
  });

  return {
    showCookieModal,
    showUpdateModal,
    updateVersion,
    updateNotes,
    showYtdlpSetupModal,
    showDenoSetupModal,
    showFfmpegSetupModal,
    toolUpdates,
  };
});
