<script setup lang="ts">
import { open } from "@tauri-apps/plugin-dialog";
import { useSettingStore } from "@/stores/setting";
import { useI18n } from "vue-i18n";

const { t } = useI18n();
const settingStore = useSettingStore();

/** 打开文件夹选择对话框 */
const handleSelectDir = async () => {
  const selected = await open({
    directory: true,
    multiple: false,
    title: t("ffmpegDir.selectDir"),
  });
  if (selected) {
    settingStore.ffmpegDir = selected as string;
  }
};

const handleClear = () => {
  settingStore.ffmpegDir = "";
};
</script>

<template>
  <n-card :title="$t('ffmpegDir.title')" size="small">
    <n-flex vertical :size="12">
      <n-text depth="3" style="font-size: 13px">
        {{ $t("ffmpegDir.desc") }}
      </n-text>
      <n-flex align="center" :size="8">
        <n-input
          :value="settingStore.ffmpegDir"
          :placeholder="$t('ffmpegDir.notSet')"
          size="small"
          readonly
          style="flex: 1"
        />
        <n-button v-if="settingStore.ffmpegDir" size="small" @click="handleClear">
          <template #icon>
            <n-icon>
              <icon-mdi-close />
            </n-icon>
          </template>
          {{ $t("common.clear") }}
        </n-button>
        <n-button size="small" @click="handleSelectDir">
          <template #icon>
            <n-icon>
              <icon-mdi-folder-open-outline />
            </n-icon>
          </template>
          {{ $t("common.select") }}
        </n-button>
      </n-flex>
    </n-flex>
  </n-card>
</template>
