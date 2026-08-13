<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { open as openExternal } from "@tauri-apps/plugin-shell";
import IconMdiDownloadOutline from "~icons/mdi/download-outline";
import IconMdiCursorDefaultClick from "~icons/mdi/cursor-default-click";
import IconMdiOpenInNew from "~icons/mdi/open-in-new";
import IconMdiFolderOpenOutline from "~icons/mdi/folder-open-outline";
import { useI18n } from "vue-i18n";

const { t } = useI18n();
const REPO_URL = "https://github.com/imsyy/yt-dlp-gui/tree/master/browser-extension";
const SUPPORTED_SITES_URL = "https://github.com/yt-dlp/yt-dlp/blob/master/supportedsites.md";
const preparing = ref(false);

const prepareExtension = async () => {
  preparing.value = true;
  try {
    await invoke("reveal_browser_extension");
    window.$message.success(t("browserExt.packageSaved"));
  } catch (error: unknown) {
    window.$message.error(t("common.saveFailed", { e: error }));
  } finally {
    preparing.value = false;
  }
};
</script>

<template>
  <n-flex vertical :size="12">
    <n-flex align="center" justify="space-between" :wrap="false">
      <n-flex align="center" :size="8">
        <n-button strong secondary size="small" @click="$router.back()">
          <template #icon><n-icon><icon-mdi-arrow-left /></n-icon></template>
          {{ $t("common.back") }}
        </n-button>
        <n-text strong style="font-size: 15px">{{ $t("browserExt.title") }}</n-text>
      </n-flex>
      <n-flex :size="8" :wrap="false">
        <n-button text size="small" @click="openExternal(SUPPORTED_SITES_URL)">
          <template #icon><n-icon><icon-mdi-open-in-new /></n-icon></template>
          {{ $t("browserExt.supportedHeading") }}
        </n-button>
        <n-button text size="small" @click="openExternal(REPO_URL)">
          <template #icon><n-icon><icon-mdi-open-in-new /></n-icon></template>
          {{ $t("browserExt.viewSource") }}
        </n-button>
        <n-button type="primary" size="small" :loading="preparing" @click="prepareExtension">
          <template #icon><n-icon><icon-mdi-folder-open-outline /></n-icon></template>
          {{ $t("browserExt.openLocalFolder") }}
        </n-button>
      </n-flex>
    </n-flex>

    <n-alert type="info" :bordered="false">{{ $t("browserExt.intro") }}</n-alert>

    <n-grid cols="1 720:2" :x-gap="12" :y-gap="12">
      <n-grid-item>
        <n-card size="small" :title="$t('browserExt.installHeading')">
          <template #header-extra><n-icon><icon-mdi-download-outline /></n-icon></template>
          <ol class="steps">
            <li v-for="index in 4" :key="index">{{ $t(`browserExt.install${index}`) }}</li>
          </ol>
        </n-card>
      </n-grid-item>
      <n-grid-item>
        <n-card size="small" :title="$t('browserExt.usageHeading')">
          <template #header-extra><n-icon><icon-mdi-cursor-default-click /></n-icon></template>
          <ul class="steps">
            <li v-for="index in 3" :key="index">{{ $t(`browserExt.usage${index}`) }}</li>
          </ul>
        </n-card>
      </n-grid-item>
    </n-grid>

  </n-flex>
</template>

<style scoped>
.steps {
  margin: 0;
  padding-left: 20px;
  line-height: 1.8;
}
</style>
