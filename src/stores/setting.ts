import { defineStore } from "pinia";

export const useSettingStore = defineStore("setting", () => {
  // 主题模式: auto 跟随系统, light 亮色, dark 暗色
  const themeMode = ref<"auto" | "light" | "dark">("auto");

  return {
    themeMode,
  };
}, {
  persist: true,
});
