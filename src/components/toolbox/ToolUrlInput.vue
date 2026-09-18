<script setup lang="ts">
import { readText } from "@tauri-apps/plugin-clipboard-manager";
import { isValidUrl } from "@/utils/validate";
import { useI18n } from "vue-i18n";

/** 工具页共用的 URL 输入行：输入框 + 粘贴按钮 */
const modelValue = defineModel<string>({ required: true });
const { t } = useI18n();

const handlePaste = async () => {
  try {
    const text = await readText();
    const trimmed = text.trim();
    if (!trimmed) {
      window.$message.warning(t("clipboard.empty"));
      return;
    }
    if (!isValidUrl(trimmed)) {
      window.$message.warning(t("clipboard.invalidUrl"));
      return;
    }
    modelValue.value = trimmed;
    window.$message.success(t("clipboard.pasteSuccess"));
  } catch {
    window.$message.warning(t("clipboard.readFailed"));
  }
};
</script>

<template>
  <n-flex :size="8" :wrap="false">
    <n-input
      v-model:value="modelValue"
      :placeholder="$t('home.inputPlaceholder')"
      clearable
      style="flex: 1"
    />
    <n-button strong secondary @click="handlePaste">
      <template #icon>
        <n-icon><icon-mdi-content-paste /></n-icon>
      </template>
      {{ $t("common.paste") }}
    </n-button>
  </n-flex>
</template>
