<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { check } from "@tauri-apps/plugin-updater";
import { useSettingStore } from "@/stores/setting";
import { useStatusStore } from "@/stores/status";
import { useI18n } from "vue-i18n";
import { localeEntries } from "@/locales";
import { getVersion } from "@tauri-apps/api/app";

const { t } = useI18n();
const settingStore = useSettingStore();
const statusStore = useStatusStore();
const appVersion = ref("");

const platform = ref("");
const platformLabel = computed(() => {
  const map: Record<string, string> = {
    windows: "Windows",
    macos: "macOS",
    linux: "Linux",
  };
  return map[platform.value] || platform.value;
});

const localeOptions = localeEntries.map((e) => ({ label: `${e.flag} ${e.label}`, value: e.code }));

const themeModeOptions = computed(() => [
  { label: t("settings.themeAuto"), value: "auto" },
  { label: t("settings.themeLight"), value: "light" },
  { label: t("settings.themeDark"), value: "dark" },
]);

const concurrentFragmentsOptions = computed(() => [
  { label: t("settings.disabled"), value: 0 },
  { label: "2", value: 2 },
  { label: "4", value: 4 },
  { label: "8", value: 8 },
  { label: "16", value: 16 },
]);

const maxConcurrentOptions = computed(() => [
  { label: t("settings.unlimited"), value: 0 },
  { label: "1", value: 1 },
  { label: "2", value: 2 },
  { label: "3", value: 3 },
  { label: "5", value: 5 },
]);

const notifyModeOptions = computed(() => [
  { label: t("settings.noNotification"), value: "none" },
  { label: t("settings.inApp"), value: "app" },
  { label: t("settings.systemNotification"), value: "system" },
  { label: t("settings.all"), value: "all" },
]);

const applyYoutubeExtractorArgs = async () => {
  await invoke("set_youtube_extractor_args", {
    poToken: settingStore.youtubePoToken,
    visitorData: settingStore.youtubeVisitorData,
  });
};

/** 检查应用更新 */
const appUpdateChecking = ref(false);

const handleCheckAppUpdate = async () => {
  appUpdateChecking.value = true;
  try {
    const update = await check();
    if (update) {
      statusStore.updateVersion = update.version;
      statusStore.updateNotes = update.body || "";
      statusStore.showUpdateModal = true;
    } else {
      window.$message.success(t("settings.appIsLatest"));
    }
  } catch (e: unknown) {
    window.$message.error(t("settings.appUpdateFailed", { e }));
  } finally {
    appUpdateChecking.value = false;
  }
};

onMounted(async () => {
  platform.value = await invoke<string>("get_platform");
  appVersion.value = await getVersion();
  await applyYoutubeExtractorArgs();
});

// PO Token / visitor_data 变更时同步到后端；不刷新状态（无需重新探测 yt-dlp）。
watch(
  () => [settingStore.youtubePoToken, settingStore.youtubeVisitorData],
  async () => {
    await applyYoutubeExtractorArgs();
  },
);
</script>

<template>
  <div class="settings-page">
    <n-flex align="center" justify="space-between" style="margin-bottom: 20px">
      <n-h2 style="margin: 0">{{ $t("settings.title") }}</n-h2>
    </n-flex>

    <ToolManager />

    <n-card :title="$t('settings.youtubeAdvanced')" size="small" class="section-card">
      <n-flex vertical :size="12">
        <n-text depth="3" style="font-size: 13px">
          {{ $t("settings.youtubeAdvancedDesc") }}
        </n-text>
        <div class="info-list">
          <div class="info-row">
            <span class="info-label">{{ $t("settings.youtubePoToken") }}</span>
            <n-input
              v-model:value="settingStore.youtubePoToken"
              :placeholder="$t('settings.youtubePoTokenPlaceholder')"
              size="small"
              clearable
              style="flex: 1; max-width: 480px"
            />
          </div>
          <div class="info-row">
            <span class="info-label">{{ $t("settings.youtubeVisitorData") }}</span>
            <n-input
              v-model:value="settingStore.youtubeVisitorData"
              :placeholder="$t('settings.youtubeVisitorDataPlaceholder')"
              size="small"
              clearable
              style="flex: 1; max-width: 480px"
            />
          </div>
        </div>
      </n-flex>
    </n-card>

    <n-card :title="$t('settings.appearance')" size="small" class="section-card">
      <div class="info-list">
        <div class="info-row">
          <span class="info-label">{{ $t("settings.language") }}</span>
          <n-select
            v-model:value="settingStore.locale"
            :options="localeOptions"
            style="width: 120px"
            size="small"
          />
        </div>
        <div class="info-row">
          <span class="info-label">{{ $t("settings.themeMode") }}</span>
          <n-select
            v-model:value="settingStore.themeMode"
            :options="themeModeOptions"
            style="width: 120px"
            size="small"
          />
        </div>
        <div class="info-row">
          <span class="info-label">{{ $t("settings.closeToTray") }}</span>
          <n-switch v-model:value="settingStore.closeToTray" />
        </div>
        <div class="info-row">
          <span class="info-label">{{ $t("settings.autoCheckUpdate") }}</span>
          <n-switch v-model:value="settingStore.autoCheckUpdate" />
        </div>
      </div>
    </n-card>

    <n-card :title="$t('settings.personalization')" size="small" class="section-card">
      <div class="info-list">
        <div class="info-row">
          <span class="info-label">{{ $t("settings.showTaskbarProgress") }}</span>
          <n-switch v-model:value="settingStore.showTaskbarProgress" />
        </div>
      </div>
    </n-card>

    <CookieCard class="section-card" />

    <DownloadDirCard class="section-card" />

    <ProxyCard class="section-card" />

    <n-card :title="$t('settings.downloadOptions')" size="small" class="section-card">
      <n-flex vertical :size="12">
        <div class="info-list">
          <div class="info-row">
            <span class="info-label">{{ $t("settings.concurrentFragments") }}</span>
            <n-select
              v-model:value="settingStore.concurrentFragments"
              :options="concurrentFragmentsOptions"
              size="small"
              style="width: 120px"
            />
          </div>
        </div>
        <div class="info-list">
          <div class="info-row">
            <span class="info-label">{{ $t("settings.maxConcurrentDownloads") }}</span>
            <n-select
              v-model:value="settingStore.maxConcurrentDownloads"
              :options="maxConcurrentOptions"
              size="small"
              style="width: 120px"
            />
          </div>
        </div>
        <div class="info-list">
          <div class="info-row">
            <span class="info-label">{{ $t("settings.downloadNotification") }}</span>
            <n-select
              v-model:value="settingStore.notifyMode"
              :options="notifyModeOptions"
              size="small"
              style="width: 120px"
            />
          </div>
        </div>
        <div class="info-list">
          <div class="info-row">
            <span class="info-label">{{ $t("settings.noOverwrites") }}</span>
            <n-switch v-model:value="settingStore.noOverwrites" />
          </div>
        </div>
        <div class="info-list">
          <div class="info-row">
            <n-tooltip placement="right" :style="{ maxWidth: '320px' }">
              <template #trigger>
                <span class="info-label">{{ $t("settings.defaultFfmpegArgs") }}</span>
              </template>
              {{ $t("settings.defaultFfmpegArgsHint") }}
            </n-tooltip>
            <n-input
              v-model:value="settingStore.defaultFfmpegArgs"
              :placeholder="$t('detail.ffmpegArgsPlaceholder')"
              size="small"
              clearable
              style="width: min(480px, 64vw)"
            />
          </div>
        </div>
      </n-flex>
    </n-card>

    <n-card :title="$t('settings.about')" size="small" class="section-card">
      <template #header-extra>
        <n-button
          :loading="appUpdateChecking"
          strong
          secondary
          round
          size="small"
          @click="handleCheckAppUpdate"
        >
          {{ $t("settings.checkAppUpdate") }}
        </n-button>
      </template>
      <n-flex vertical :size="8">
        <n-text depth="3" style="font-size: 13px">
          {{ $t("settings.aboutDesc") }}
        </n-text>
        <div class="info-list">
          <div class="info-row">
            <span class="info-label">{{ $t("settings.version") }}</span>
            <n-text code>v{{ appVersion }}</n-text>
          </div>
          <div class="info-row">
            <span class="info-label">{{ $t("settings.platform") }}</span>
            <n-text code>{{ platformLabel }}</n-text>
          </div>
          <div class="info-row">
            <span class="info-label">{{ $t("settings.license") }}</span>
            <n-text code>MIT</n-text>
          </div>
          <div class="info-row">
            <span class="info-label">{{ $t("settings.repository") }}</span>
            <n-button
              text
              tag="a"
              href="https://github.com/imsyy/yt-dlp-gui"
              target="_blank"
              size="tiny"
            >
              GitHub
            </n-button>
          </div>
        </div>
      </n-flex>
    </n-card>
  </div>
</template>

<style scoped lang="scss">
.settings-page {
  max-width: 100%;
}

.section-card {
  margin-bottom: 12px;
}

.info-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.info-row {
  display: flex;
  align-items: center;
  font-size: 13px;
  min-height: 28px;

  &::before {
    order: 1;
    content: "";
    flex: 1;
    border-bottom: 1px dashed var(--n-border-color, #e0e0e6);
    margin: 0 8px;
    min-width: 20px;
  }

  > :last-child {
    order: 2;
    flex-shrink: 0;
  }
}

.info-label {
  flex-shrink: 0;
  order: 0;
}
</style>
