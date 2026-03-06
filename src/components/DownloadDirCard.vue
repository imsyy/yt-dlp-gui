<script setup lang="ts">
import { open } from "@tauri-apps/plugin-dialog";
import { useSettingStore } from "@/stores/setting";

const settingStore = useSettingStore();

/** 打开文件夹选择对话框 */
const handleSelectDir = async () => {
  const selected = await open({
    directory: true,
    multiple: false,
    title: "选择下载目录",
  });
  if (selected) {
    settingStore.downloadDir = selected as string;
  }
};
</script>

<template>
  <n-card title="下载目录" size="small">
    <n-flex align="center" :size="8">
      <n-input
        :value="settingStore.downloadDir"
        placeholder="未设置下载目录"
        size="small"
        readonly
        style="flex: 1"
      />
      <n-button size="small" @click="handleSelectDir">
        <template #icon>
          <n-icon>
            <icon-mdi-folder-open-outline />
          </n-icon>
        </template>
        选择
      </n-button>
    </n-flex>
  </n-card>
</template>
