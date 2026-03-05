<!-- 全局配置 -->
<template>
  <n-config-provider
    :locale="zhCN"
    :date-locale="dateZhCN"
    :theme="theme"
    :theme-overrides="themeOverrides"
    abstract
    inline-theme-disabled
    preflight-style-disabled
  >
    <n-global-style />
    <n-loading-bar-provider>
      <n-dialog-provider>
        <n-notification-provider>
          <n-message-provider :max="1" placement="bottom">
            <n-modal-provider>
              <slot />
              <NaiveProviderContent />
            </n-modal-provider>
          </n-message-provider>
        </n-notification-provider>
      </n-dialog-provider>
    </n-loading-bar-provider>
  </n-config-provider>
</template>

<script setup lang="ts">
import {
  zhCN,
  dateZhCN,
  darkTheme,
  useOsTheme,
  useLoadingBar,
  useModal,
  useDialog,
  useMessage,
  useNotification,
  GlobalThemeOverrides,
} from "naive-ui";
import { useSettingStore } from "@/stores/setting";

// 设置
const settingStore = useSettingStore();

// 操作系统主题
const osTheme = useOsTheme();

// 全局主题
const themeOverrides = shallowRef<GlobalThemeOverrides>({
  common:{
    borderRadius:"8px"
  }
});

// 获取明暗模式
const theme = computed(() => {
  return settingStore.themeMode === "auto"
    ? // 跟随系统
      osTheme.value === "dark"
      ? darkTheme
      : null
    : // 自定义
      settingStore.themeMode === "dark"
      ? darkTheme
      : null;
});

// 挂载工具
const NaiveProviderContent = defineComponent({
  setup() {
    // 进度条
    window.$loadingBar = useLoadingBar();
    // 通知
    window.$notification = useNotification();
    // 信息
    window.$message = useMessage();
    // 对话框
    window.$dialog = useDialog();
    // 模态框
    window.$modal = useModal();
  },
  render() {
    return h("div", { className: "main-tools" });
  },
});
</script>
