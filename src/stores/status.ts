import { defineStore } from "pinia";

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

  return {
    showCookieModal,
    showUpdateModal,
    updateVersion,
    updateNotes,
    showYtdlpSetupModal,
    showDenoSetupModal,
  };
});
