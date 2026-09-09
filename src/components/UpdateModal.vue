<script setup lang="ts">
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { getVersion } from "@tauri-apps/api/app";
import { useStatusStore } from "@/stores/status";
import { useI18n } from "vue-i18n";

const { t } = useI18n();
const statusStore = useStatusStore();

const currentVersion = ref("");
const downloading = ref(false);
const progress = ref(0);
const contentLength = ref(0);
const downloaded = ref(0);

onMounted(async () => {
  currentVersion.value = await getVersion();
});

const handleUpdate = async () => {
  downloading.value = true;
  progress.value = 0;
  try {
    const update = await check();
    if (!update) return;

    await update.downloadAndInstall((event) => {
      if (event.event === "Started") {
        contentLength.value = event.data.contentLength || 0;
        downloaded.value = 0;
      } else if (event.event === "Progress") {
        downloaded.value += event.data.chunkLength;
        if (contentLength.value > 0) {
          progress.value = Math.round((downloaded.value / contentLength.value) * 100);
        }
      }
    });

    window.$message.success(t("settings.appUpdateSuccess"));
    await relaunch();
  } catch (e: unknown) {
    window.$message.error(t("settings.appUpdateFailed", { e }));
  } finally {
    downloading.value = false;
  }
};

const handleClose = () => {
  if (!downloading.value) {
    statusStore.showUpdateModal = false;
  }
};
</script>

<template>
  <n-modal
    v-model:show="statusStore.showUpdateModal"
    preset="card"
    :title="$t('settings.appNewVersionFound')"
    size="small"
    :bordered="false"
    :closable="!downloading"
    :mask-closable="!downloading"
    :style="{ width: '420px' }"
    @after-leave="progress = 0"
  >
    <n-flex vertical :size="16">
      <n-flex align="center" :size="8">
        <n-tag size="small" round>v{{ currentVersion }}</n-tag>
        <icon-mdi-arrow-right style="font-size: 14px; color: var(--n-text-color-3)" />
        <n-tag type="primary" :bordered="false" size="small" round>
          v{{ statusStore.updateVersion }}
        </n-tag>
      </n-flex>

      <n-flex v-if="statusStore.updateNotes" vertical :size="8">
        <n-text depth="3" style="font-size: 13px">
          {{ $t("settings.appUpdateNotes") }}
        </n-text>
        <n-scrollbar style="max-height: 200px">
          <n-text style="font-size: 13px; white-space: pre-wrap; line-height: 1.6">
            {{ statusStore.updateNotes }}
          </n-text>
        </n-scrollbar>
      </n-flex>

      <n-collapse-transition :show="downloading">
        <n-flex vertical :size="4">
          <n-text depth="3" style="font-size: 12px">
            {{ $t("settings.appUpdating") }}
          </n-text>
          <n-progress
            type="line"
            :percentage="progress"
            :processing="true"
            :height="20"
            :border-radius="4"
            indicator-placement="inside"
          />
        </n-flex>
      </n-collapse-transition>
    </n-flex>

    <template #footer>
      <n-flex justify="end" :size="8">
        <n-button :disabled="downloading" strong secondary @click="handleClose">
          {{ $t("settings.appUpdateLater") }}
        </n-button>
        <n-button type="primary" :loading="downloading" strong secondary @click="handleUpdate">
          {{ $t("settings.appUpdateNow") }}
        </n-button>
      </n-flex>
    </template>
  </n-modal>
</template>
