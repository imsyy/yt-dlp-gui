<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import IconMdiDownload from "~icons/mdi/download";
import IconMdiCookieCog from "~icons/mdi/cookie-cog";
import IconMdiLanguageJavascript from "~icons/mdi/language-javascript";
import IconMdiMovieOpenCog from "~icons/mdi/movie-open-cog";
import { useI18n } from "vue-i18n";
import { useDownloadStore } from "@/stores/download";
import { useSettingStore } from "@/stores/setting";
import { useStatusStore } from "@/stores/status";
import type { ToolOperationProgress, ToolStatus } from "@/types";
import type { Component } from "vue";

type ToolKey = "yt-dlp" | "deno" | "ffmpeg";

const { t } = useI18n();
const router = useRouter();
const settingStore = useSettingStore();
const statusStore = useStatusStore();
const downloadStore = useDownloadStore();
const statuses = reactive<Record<ToolKey, ToolStatus | null>>({
  "yt-dlp": null,
  deno: null,
  ffmpeg: null,
});

const tools: { key: ToolKey; label: string; command: string; icon: Component }[] = [
  { key: "yt-dlp", label: "yt-dlp", command: "get_ytdlp_status", icon: IconMdiDownload },
  {
    key: "deno",
    label: "Deno",
    command: "get_deno_status",
    icon: IconMdiLanguageJavascript,
  },
  { key: "ffmpeg", label: "FFmpeg", command: "get_ffmpeg_status", icon: IconMdiMovieOpenCog },
];

const speedUnits: Record<string, number> = {
  "b/s": 1,
  "kb/s": 1_000,
  "kib/s": 1_024,
  "mb/s": 1_000_000,
  "mib/s": 1_048_576,
  "gb/s": 1_000_000_000,
  "gib/s": 1_073_741_824,
};

const parseSpeed = (speed: string) => {
  const match = speed.trim().match(/^([\d.]+)\s*([kmgt]?i?b\/s)$/i);
  if (!match) return 0;
  return Number(match[1]) * (speedUnits[match[2].toLowerCase()] || 0);
};

const formatSpeed = (bytesPerSecond: number) => {
  if (bytesPerSecond <= 0) return "0 B/s";
  const units = ["B/s", "KiB/s", "MiB/s", "GiB/s"];
  let value = bytesPerSecond;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value >= 100 || unit === 0 ? value.toFixed(0) : value.toFixed(1)} ${units[unit]}`;
};

const totalSpeed = computed(() =>
  formatSpeed(
    downloadStore.tasks
      .filter((task) => task.status === "downloading" || task.status === "postprocessing")
      .reduce((total, task) => total + parseSpeed(task.speed), 0),
  ),
);

const refreshStatuses = async () => {
  await invoke("set_tool_sources", {
    ytdlp: settingStore.ytdlpSource,
    deno: settingStore.denoSource,
    ffmpeg: settingStore.ffmpegSource,
  });
  await Promise.all(
    tools.map(async (tool) => {
      try {
        statuses[tool.key] = await invoke<ToolStatus>(tool.command);
      } catch {
        statuses[tool.key] = null;
      }
    }),
  );
};

const sourceText = (tool: ToolKey) => {
  const status = statuses[tool];
  if (!status) return "—";
  if (status.source === "managed") return t("settings.sourceManaged");
  if (status.source === "system") return t("settings.sourceSystem");
  return t("settings.cliManaged");
};

let unlistenProgress: (() => void) | null = null;

watch(
  () => [settingStore.ytdlpSource, settingStore.denoSource, settingStore.ffmpegSource],
  () => void refreshStatuses(),
  { immediate: true },
);

onMounted(async () => {
  unlistenProgress = await listen<ToolOperationProgress>("tool-operation-progress", (event) => {
    if (event.payload.stage === "complete") void refreshStatuses();
  });
});

onUnmounted(() => unlistenProgress?.());
</script>

<template>
  <n-layout-footer
    position="absolute"
    bordered
    class="status-bar"
    :aria-label="$t('statusBar.title')"
  >
    <n-flex align="center" justify="space-between" :wrap="false" class="status-list">
      <n-button
        :focusable="false"
        text
        size="tiny"
        class="download-summary"
        @click="router.push({ name: 'downloads' })"
      >
        <template #icon>
          <n-icon><icon-mdi-download /></n-icon>
        </template>
        {{ $t("statusBar.activeDownloads", { count: downloadStore.activeCount }) }}
        <n-divider vertical />
        {{ totalSpeed }}
      </n-button>
      <n-flex align="center" :size="12" :wrap="false">
        <n-tooltip trigger="hover">
          <template #trigger>
            <n-button
              text
              size="tiny"
              :focusable="false"
              class="tool-trigger"
              :aria-label="$t('cookie.settings')"
              @click="statusStore.showCookieModal = true"
            >
              <n-icon size="18"><icon-mdi-cookie-cog /></n-icon>
            </n-button>
          </template>
          {{ $t("cookie.settings") }}
        </n-tooltip>
        <n-divider vertical class="status-divider" />
        <n-popover
          v-for="tool in tools"
          :key="tool.key"
          :show-arrow="false"
          trigger="click"
          placement="top-end"
          :width="240"
        >
          <template #trigger>
            <n-button
              text
              size="tiny"
              :focusable="false"
              class="tool-trigger"
              :aria-label="tool.label"
            >
              <n-badge
                dot
                type="error"
                :show="statuses[tool.key]?.installed === false"
                :offset="[-1, 2]"
              >
                <n-icon size="18">
                  <component :is="tool.icon" />
                </n-icon>
              </n-badge>
            </n-button>
          </template>
          <n-flex vertical :size="8">
            <n-flex align="center" justify="space-between" :wrap="false" :size="10">
              <n-text style="font-size: 15px">{{ tool.label }}</n-text>
              <n-flex align="center" :size="6" :wrap="false">
                <n-tag
                  size="small"
                  round
                  :type="statuses[tool.key]?.installed ? 'success' : 'error'"
                  :bordered="false"
                >
                  {{
                    statuses[tool.key]
                      ? statuses[tool.key]?.installed
                        ? $t("settings.installed")
                        : $t("settings.notInstalled")
                      : $t("statusBar.checking")
                  }}
                </n-tag>
                <n-tooltip trigger="hover">
                  <template #trigger>
                    <n-button
                      text
                      size="small"
                      :focusable="false"
                      @click="router.push({ name: 'settings' })"
                    >
                      <n-icon><icon-mdi-cog /></n-icon>
                    </n-button>
                  </template>
                  {{ $t("setup.goToSettings") }}
                </n-tooltip>
              </n-flex>
            </n-flex>
            <div class="tool-field">
              <n-text depth="3" size="small">{{ $t("settings.version") }}</n-text>
              <n-text>{{ statuses[tool.key]?.version || "—" }}</n-text>
            </div>
            <div class="tool-field">
              <n-text depth="3" size="small">{{ $t("statusBar.source") }}</n-text>
              <n-text>{{ sourceText(tool.key) }}</n-text>
            </div>
          </n-flex>
        </n-popover>
      </n-flex>
    </n-flex>
  </n-layout-footer>
</template>

<style scoped lang="scss">
.status-bar {
  z-index: 10;
  height: 32px;
  padding: 0 12px;
  font-size: 12px;
}

.status-list {
  height: 100%;
}

.download-summary {
  font-variant-numeric: tabular-nums;
}

.tool-trigger {
  width: 24px;
  height: 24px;
}

.status-divider {
  margin: 0;
}

.tool-field {
  display: flex;
  flex-direction: column;
  gap: 2px;
}
</style>
