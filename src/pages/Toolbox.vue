<script setup lang="ts">
import { readText } from "@tauri-apps/plugin-clipboard-manager";
import { isValidUrl } from "@/utils/validate";
import { useI18n } from "vue-i18n";

const { t } = useI18n();
const toolUrl = ref("");
provide("toolUrl", toolUrl);

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
    toolUrl.value = trimmed;
    window.$message.success(t("clipboard.pasteSuccess"));
  } catch {
    window.$message.warning(t("clipboard.readFailed"));
  }
};
</script>

<template>
  <n-flex vertical :size="16" class="toolbox-page">
    <n-flex :size="8">
      <n-input
        v-model:value="toolUrl"
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

    <router-view v-slot="{ Component: RouteComponent }">
      <Transition name="fade-slide" mode="out-in">
        <component :is="RouteComponent" />
      </Transition>
    </router-view>
  </n-flex>
</template>

<style scoped lang="scss">
.toolbox-page {
  height: 100%;
  overflow-y: auto;
  padding: 16px;
}
</style>
