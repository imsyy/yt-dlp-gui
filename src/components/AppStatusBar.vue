<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { formatError } from "@/utils/format";
import IconMdiDownload from "~icons/mdi/download";
import IconMdiCookieCog from "~icons/mdi/cookie-cog";
import IconMdiLanguageJavascript from "~icons/mdi/language-javascript";
import IconMdiMovieOpenCog from "~icons/mdi/movie-open-cog";
import { useI18n } from "vue-i18n";
import { useDownloadStore } from "@/stores/download";
import { useSettingStore } from "@/stores/setting";
import { useStatusStore } from "@/stores/status";
import type { ToolTaskState } from "@/composables/useToolTask";
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
  await invoke("set_ytdlp_channel", { channel: settingStore.ytdlpChannel }).catch(() => {});
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

/** 工具图标是否亮点：未安装，或检测到可用更新 */
const showToolDot = (tool: ToolKey) =>
  statuses[tool]?.installed === false || statusStore.toolUpdates[tool]?.updateAvailable === true;

/** 亮点类型：未安装用 error 红点，有可用更新用 warning 黄点 */
const toolDotType = (tool: ToolKey): "error" | "warning" =>
  statuses[tool]?.installed === false ? "error" : "warning";

/** 弹窗标签：与设置页一致，有更新时显示"有更新" */
const toolTagType = (tool: ToolKey) => {
  if (statusStore.toolUpdates[tool]?.updateAvailable === true) return "warning";
  return statuses[tool]?.installed ? "success" : "error";
};

const toolTagText = (tool: ToolKey) => {
  if (!statuses[tool]) return t("statusBar.checking");
  if (statusStore.toolUpdates[tool]?.updateAvailable === true) return t("settings.updateAvailable");
  return statuses[tool]?.installed ? t("settings.installed") : t("settings.notInstalled");
};

/** 正在运行的工具任务，用于底栏全局指示 */
const runningTools = ref<string[]>([]);
const showRunningTools = ref(false);

/** 运行中任务的展示名与跳转路由 */
const toolTaskMeta: Record<string, { labelKey: string; route: string }> = {
  thumbnail: { labelKey: "toolbox.thumbnailTitle", route: "toolbox-thumbnail" },
  subtitles: { labelKey: "toolbox.subtitlesTitle", route: "toolbox-subtitles" },
  livechat: { labelKey: "toolbox.livechatTitle", route: "toolbox-livechat" },
  chapters: { labelKey: "toolbox.chaptersTitle", route: "toolbox-chapters" },
  comments: { labelKey: "toolbox.commentsTitle", route: "toolbox-comments" },
};

const toolTaskLabel = (toolId: string) => t(toolTaskMeta[toolId]?.labelKey ?? "common.unknown");

const refreshRunningTools = async () => {
  try {
    runningTools.value = await invoke<string[]>("tool_get_running_tasks");
  } catch {
    runningTools.value = [];
  }
};

const goToTool = (toolId: string) => {
  showRunningTools.value = false;
  router.push({ name: toolTaskMeta[toolId]?.route ?? "toolbox" });
};

let unlistenProgress: (() => void) | null = null;
let unlistenToolTask: (() => void) | null = null;

watch(
  () => [
    settingStore.ytdlpSource,
    settingStore.denoSource,
    settingStore.ffmpegSource,
    settingStore.ytdlpChannel,
  ],
  () => void refreshStatuses(),
  { immediate: true },
);

onMounted(async () => {
  unlistenProgress = await listen<ToolOperationProgress>("tool-operation-progress", (event) => {
    if (event.payload.stage === "complete") void refreshStatuses();
  });
  // 工具任务状态变化：刷新底栏指示，并统一在这里发出通知
  // （底栏常驻，用户不在该工具页时也能收到取消/失败提示）
  unlistenToolTask = await listen<ToolTaskState>("tool-task-state-changed", ({ payload }) => {
    void refreshRunningTools();
    if (payload.status === "cancelled") window.$message.info(t("toolbox.taskCancelled"));
    else if (payload.status === "failed" && payload.error) {
      window.$message.error(formatError(payload.error));
    }
  });
  void refreshRunningTools();
});

onUnmounted(() => {
  unlistenProgress?.();
  unlistenToolTask?.();
});
</script>

<template>
  <n-layout-footer
    position="absolute"
    bordered
    class="status-bar"
    :aria-label="$t('statusBar.title')"
  >
    <n-flex align="center" justify="space-between" :wrap="false" class="status-list">
      <n-flex align="center" :size="12" :wrap="false">
        <n-button
          :focusable="false"
          text
          size="tiny"
          class="status-summary"
          @click="router.push({ name: 'downloads' })"
        >
          <template #icon>
            <n-icon><icon-mdi-download /></n-icon>
          </template>
          {{ $t("statusBar.activeDownloads", { count: downloadStore.activeCount }) }}
          <n-divider vertical />
          {{ totalSpeed }}
        </n-button>
        <n-divider vertical class="status-divider" />
        <n-popover
          v-model:show="showRunningTools"
          :show-arrow="false"
          trigger="click"
          placement="top-start"
          :width="240"
        >
          <template #trigger>
            <n-button
              :focusable="false"
              text
              size="tiny"
              class="status-summary"
              :aria-label="$t('statusBar.runningToolTasks', { count: runningTools.length })"
            >
              <template #icon>
                <n-icon><icon-mdi-toolbox /></n-icon>
              </template>
              {{ $t("statusBar.runningToolTasks", { count: runningTools.length }) }}
            </n-button>
          </template>
          <n-text v-if="runningTools.length === 0" style="font-size: 13px">
            {{ $t("statusBar.noRunningToolTasks") }}
          </n-text>
          <n-flex v-else vertical :size="10">
            <n-flex
              v-for="tool in runningTools"
              :key="tool"
              align="center"
              justify="space-between"
              :size="10"
              :wrap="false"
            >
              <n-text style="font-size: 13px">{{ toolTaskLabel(tool) }}</n-text>
              <n-button secondary strong size="small" :focusable="false" @click="goToTool(tool)">
                {{ $t("common.open") }}
              </n-button>
            </n-flex>
          </n-flex>
        </n-popover>
      </n-flex>
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
                :type="toolDotType(tool.key)"
                :show="showToolDot(tool.key)"
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
                <n-tag size="small" round :type="toolTagType(tool.key)" :bordered="false">
                  {{ toolTagText(tool.key) }}
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
              <n-text depth="3">{{ $t("settings.version") }}</n-text>
              <n-text>{{ statuses[tool.key]?.version || "—" }}</n-text>
            </div>
            <div v-if="statusStore.toolUpdates[tool.key]?.updateAvailable" class="tool-field">
              <n-text depth="3">{{ $t("settings.latestVersion") }}</n-text>
              <n-text type="warning">{{ statusStore.toolUpdates[tool.key]?.latestVersion }}</n-text>
            </div>
            <div class="tool-field">
              <n-text depth="3">{{ $t("statusBar.source") }}</n-text>
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

.status-summary {
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
