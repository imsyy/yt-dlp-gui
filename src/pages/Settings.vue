<script setup lang="ts">
import type { YtdlpStatus, DenoStatus, DownloadProgress } from "@/types";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
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

const ytdlpStatus = ref<YtdlpStatus | null>(null);
const ytdlpChecking = ref(true);
const ytdlpDownloading = ref(false);
const ytdlpDownloadPercent = ref(0);
const ytdlpUpdating = ref(false);

/** 检查 yt-dlp 安装状态与版本 */
const checkYtdlpStatus = async () => {
  ytdlpChecking.value = true;
  try {
    ytdlpStatus.value = await invoke<YtdlpStatus>("get_ytdlp_status");
  } catch (e) {
    // ignore
  } finally {
    ytdlpChecking.value = false;
  }
};

/** 下载 yt-dlp 并监听进度事件 */
const handleDownloadYtdlp = async () => {
  ytdlpDownloading.value = true;
  ytdlpDownloadPercent.value = 0;
  const unlisten = await listen<DownloadProgress>("ytdlp-download-progress", (event) => {
    ytdlpDownloadPercent.value = event.payload.percent;
  });
  try {
    await invoke("download_ytdlp");
    window.$message.success(t("settings.ytdlpDownloadComplete"));
    await checkYtdlpStatus();
  } catch (e: unknown) {
    window.$message.error(t("common.downloadFailed", { e }));
  } finally {
    unlisten();
    ytdlpDownloading.value = false;
  }
};

/** 检查并更新 yt-dlp 到最新版本 */
const handleUpdateYtdlp = async () => {
  ytdlpUpdating.value = true;
  try {
    const result = await invoke<string>("update_ytdlp");
    if (result.includes("up to date")) {
      window.$message.success(t("settings.alreadyLatest"));
    } else if (result.includes("Updated")) {
      window.$message.success(t("settings.updatedToLatest"));
      await checkYtdlpStatus();
    } else {
      window.$message.success(t("settings.alreadyLatest"));
    }
  } catch (e: unknown) {
    window.$message.error(t("settings.updateFailed", { e }));
  } finally {
    ytdlpUpdating.value = false;
  }
};

const denoStatus = ref<DenoStatus | null>(null);
const denoChecking = ref(true);
const denoDownloading = ref(false);
const denoDownloadPercent = ref(0);

/** 检查 Deno 安装状态与版本 */
const checkDenoStatus = async () => {
  denoChecking.value = true;
  try {
    denoStatus.value = await invoke<DenoStatus>("get_deno_status");
  } catch (e) {
    // ignore
  } finally {
    denoChecking.value = false;
  }
};

/** 下载 Deno 并监听进度事件 */
const handleDownloadDeno = async () => {
  denoDownloading.value = true;
  denoDownloadPercent.value = 0;
  const unlisten = await listen<DownloadProgress>("deno-download-progress", (event) => {
    denoDownloadPercent.value = event.payload.percent;
  });
  try {
    await invoke("download_deno");
    window.$message.success(t("settings.denoDownloadComplete"));
    await checkDenoStatus();
  } catch (e: unknown) {
    window.$message.error(t("common.downloadFailed", { e }));
  } finally {
    unlisten();
    denoDownloading.value = false;
  }
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

/** 刷新所有依赖状态 */
const refreshAll = () => {
  checkYtdlpStatus();
  checkDenoStatus();
};

onMounted(async () => {
  platform.value = await invoke<string>("get_platform");
  appVersion.value = await getVersion();
  refreshAll();
});
</script>

<template>
  <div class="settings-page">
    <n-flex align="center" justify="space-between" style="margin-bottom: 20px">
      <n-h2 style="margin: 0">{{ $t("settings.title") }}</n-h2>
      <n-button size="small" strong secondary @click="refreshAll">
        <template #icon>
          <n-icon>
            <icon-mdi-refresh />
          </n-icon>
        </template>
        {{ $t("common.refresh") }}
      </n-button>
    </n-flex>

    <n-card title="yt-dlp" size="small" class="section-card">
      <template #header-extra>
        <n-flex align="center" :size="8">
          <n-tag v-if="!ytdlpChecking" :type="ytdlpStatus?.installed ? 'success' : 'error'" round>
            {{ ytdlpStatus?.installed ? $t("settings.installed") : $t("settings.notInstalled") }}
          </n-tag>
          <n-button
            v-if="ytdlpStatus?.installed"
            :loading="ytdlpUpdating"
            strong
            secondary
            round
            size="small"
            @click="handleUpdateYtdlp"
          >
            {{ $t("settings.checkUpdate") }}
          </n-button>
          <n-button
            v-if="ytdlpStatus && !ytdlpStatus.installed"
            :loading="ytdlpDownloading"
            :disabled="ytdlpDownloading"
            type="primary"
            size="small"
            strong
            secondary
            round
            @click="handleDownloadYtdlp"
          >
            {{ $t("common.download") }}
          </n-button>
        </n-flex>
      </template>

      <n-spin :show="ytdlpChecking">
        <n-flex vertical :size="12">
          <n-text depth="3" style="font-size: 13px">
            {{ $t("settings.ytdlpDesc") }}
          </n-text>

          <div class="info-list">
            <div class="info-row">
              <span class="info-label">{{ $t("settings.version") }}</span>
              <n-text code>{{ ytdlpStatus?.version || "—" }}</n-text>
            </div>
            <div class="info-row">
              <span class="info-label">{{ $t("settings.path") }}</span>
              <n-ellipsis :line-clamp="1" :tooltip="{ width: 360 }">
                {{ ytdlpStatus?.path || "—" }}
              </n-ellipsis>
            </div>
          </div>

          <n-collapse-transition :show="ytdlpDownloading">
            <n-progress
              type="line"
              :percentage="Math.round(ytdlpDownloadPercent)"
              :processing="true"
              indicator-placement="inside"
              :height="20"
              :border-radius="4"
              style="margin-top: 4px"
            />
          </n-collapse-transition>
        </n-flex>
      </n-spin>
    </n-card>

    <n-card :title="$t('settings.denoTitle')" size="small" class="section-card">
      <template #header-extra>
        <n-flex align="center" :size="8">
          <n-tag v-if="!denoChecking" :type="denoStatus?.installed ? 'success' : 'error'" round>
            {{ denoStatus?.installed ? $t("settings.installed") : $t("settings.notInstalled") }}
          </n-tag>
          <n-button
            v-if="denoStatus && !denoStatus.installed"
            :loading="denoDownloading"
            :disabled="denoDownloading"
            type="primary"
            size="small"
            strong
            secondary
            round
            @click="handleDownloadDeno"
          >
            {{ $t("common.download") }}
          </n-button>
        </n-flex>
      </template>

      <n-spin :show="denoChecking">
        <n-flex vertical :size="12">
          <n-text depth="3" style="font-size: 13px">
            {{ $t("settings.denoDesc") }}
          </n-text>

          <div class="info-list">
            <div class="info-row">
              <span class="info-label">{{ $t("settings.version") }}</span>
              <n-text code>{{ denoStatus?.version || "—" }}</n-text>
            </div>
            <div class="info-row">
              <span class="info-label">{{ $t("settings.path") }}</span>
              <n-ellipsis :line-clamp="1" :tooltip="{ width: 360 }">
                {{ denoStatus?.path || "—" }}
              </n-ellipsis>
            </div>
          </div>

          <n-collapse-transition :show="denoDownloading">
            <n-progress
              type="line"
              :percentage="Math.round(denoDownloadPercent)"
              :processing="true"
              indicator-placement="inside"
              :height="20"
              :border-radius="4"
              style="margin-top: 4px"
            />
          </n-collapse-transition>
        </n-flex>
      </n-spin>
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
      </div>
    </n-card>

    <CookieCard class="section-card" />

    <DownloadDirCard class="section-card" />

    <n-card :title="$t('settings.downloadOptions')" size="small" class="section-card">
      <n-flex vertical :size="12">
        <div class="info-list">
          <div class="info-row">
            <span class="info-label">{{ $t("settings.proxy") }}</span>
            <n-input
              v-model:value="settingStore.proxy"
              :placeholder="$t('settings.proxyPlaceholder')"
              size="small"
              clearable
              style="width: 220px"
            />
          </div>
        </div>
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
