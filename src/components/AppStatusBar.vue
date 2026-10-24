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
import type {
  ChannelEnrichProgressPayload,
  ChannelSyncProgressPayload,
  ToolOperationProgress,
  ToolStatus,
} from "@/types";
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

const runtimeLabels = { deno: "Deno", node: "Node.js", bun: "Bun", quickjs: "QuickJS" };
const tools = computed<{ key: ToolKey; label: string; command: string; icon: Component }[]>(() => [
  { key: "yt-dlp", label: "yt-dlp", command: "get_ytdlp_status", icon: IconMdiDownload },
  {
    key: "deno",
    label: runtimeLabels[settingStore.jsRuntime],
    command: "get_js_runtime_status",
    icon: IconMdiLanguageJavascript,
  },
  { key: "ffmpeg", label: "FFmpeg", command: "get_ffmpeg_status", icon: IconMdiMovieOpenCog },
]);

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

let statusRefreshId = 0;
const refreshStatuses = async () => {
  const refreshId = ++statusRefreshId;
  statuses["yt-dlp"] = null;
  statuses.deno = null;
  await invoke("set_tool_sources", {
    ytdlp: settingStore.ytdlpSource,
    deno: settingStore.denoSource,
    ffmpeg: settingStore.ffmpegSource,
    ytdlpLocalPath: settingStore.ytdlpLocalPath,
  });
  await invoke("set_ytdlp_channel", { channel: settingStore.ytdlpChannel }).catch(() => {});
  await invoke("set_js_runtime", { runtime: settingStore.jsRuntime });
  await Promise.all(
    tools.value.map(async (tool) => {
      try {
        const status = await invoke<ToolStatus>(tool.command);
        if (refreshId === statusRefreshId) statuses[tool.key] = status;
      } catch {
        if (refreshId === statusRefreshId) statuses[tool.key] = null;
      }
    }),
  );
};

const sourceText = (tool: ToolKey) => {
  const status = statuses[tool];
  if (!status) return "—";
  if (status.source === "managed") return t("settings.sourceManaged");
  if (status.source === "system") return t("settings.sourceSystem");
  if (status.source === "local") return t("settings.sourceLocal");
  return t("settings.cliManaged");
};

/** 工具图标是否亮点：未安装，或检测到可用更新 */
const hasToolUpdate = (tool: ToolKey) =>
  (tool !== "yt-dlp" || settingStore.ytdlpSource !== "local") &&
  statuses[tool]?.canUpdate === true &&
  (tool !== "deno" || settingStore.jsRuntime === "deno") &&
  statusStore.toolUpdates[tool]?.updateAvailable === true;
const showToolDot = (tool: ToolKey) => statuses[tool]?.runnable === false || hasToolUpdate(tool);

/** 亮点类型：未安装用 error 红点，有可用更新用 warning 黄点 */
const toolDotType = (tool: ToolKey): "error" | "warning" =>
  statuses[tool]?.runnable === false ? "error" : "warning";

/** 弹窗标签：与设置页一致，有更新时显示"有更新" */
const toolTagType = (tool: ToolKey) => {
  if (statuses[tool]?.runnable === false) return "error";
  if (hasToolUpdate(tool)) return "warning";
  return statuses[tool]?.runnable ? "success" : "error";
};

const toolTagText = (tool: ToolKey) => {
  if (!statuses[tool]) return t("statusBar.checking");
  if (!statuses[tool]?.installed) return t("settings.notInstalled");
  if (!statuses[tool]?.runnable) return t("settings.toolNotRunnable");
  if (hasToolUpdate(tool)) return t("settings.updateAvailable");
  return t("settings.installed");
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

const channelJobs = ref<string[]>([]);

const upsertChannelJob = (id: string, running: boolean) => {
  const index = channelJobs.value.indexOf(id);
  if (running) {
    if (index === -1) channelJobs.value.push(id);
  } else if (index !== -1) {
    channelJobs.value.splice(index, 1);
  }
};

const refreshChannelJobs = async () => {
  try {
    const [syncing, enriching] = await Promise.all([
      invoke<string[]>("channel_sync_active"),
      invoke<string[]>("channel_enrich_active"),
    ]);
    channelJobs.value = [...new Set([...syncing, ...enriching])];
  } catch {
    channelJobs.value = [];
  }
};

const totalRunningCount = computed(() => runningTools.value.length + channelJobs.value.length);
const hasRunningJobs = computed(() => totalRunningCount.value > 0);

const goToChannelArchive = () => {
  showRunningTools.value = false;
  router.push({ name: "toolbox-channel" });
};

let unlistenProgress: (() => void) | null = null;
let unlistenToolTask: (() => void) | null = null;
let unlistenChannelSync: (() => void) | null = null;
let unlistenChannelEnrich: (() => void) | null = null;

watch(
  () => [
    settingStore.ytdlpSource,
    settingStore.ytdlpLocalPath,
    settingStore.denoSource,
    settingStore.jsRuntime,
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
  // 工具任务状态变化
  unlistenToolTask = await listen<ToolTaskState>("tool-task-state-changed", ({ payload }) => {
    void refreshRunningTools();
    if (payload.status === "cancelled") window.$message.info(t("toolbox.taskCancelled"));
    else if (payload.status === "failed" && payload.error) {
      window.$message.error(formatError(payload.error));
    }
  });
  unlistenChannelSync = await listen<ChannelSyncProgressPayload>(
    "channel-sync-progress",
    ({ payload }) => {
      upsertChannelJob(payload.channelId, payload.status === "syncing");
    },
  );
  unlistenChannelEnrich = await listen<ChannelEnrichProgressPayload>(
    "channel-enrich-progress",
    ({ payload }) => {
      upsertChannelJob(payload.channelId, payload.status === "enriching");
    },
  );
  void refreshRunningTools();
  void refreshChannelJobs();
});

onUnmounted(() => {
  unlistenProgress?.();
  unlistenToolTask?.();
  unlistenChannelSync?.();
  unlistenChannelEnrich?.();
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
              :aria-label="$t('statusBar.runningToolTasks', { count: totalRunningCount })"
            >
              <template #icon>
                <n-icon><icon-mdi-toolbox /></n-icon>
              </template>
              {{ $t("statusBar.runningToolTasks", { count: totalRunningCount }) }}
            </n-button>
          </template>
          <n-text v-if="!hasRunningJobs" style="font-size: 13px">
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
            <n-flex
              v-for="jobId in channelJobs"
              :key="`channel-${jobId}`"
              align="center"
              justify="space-between"
              :size="10"
              :wrap="false"
            >
              <n-text style="font-size: 13px">{{ $t("channelArchive.title") }}</n-text>
              <n-button
                secondary
                strong
                size="small"
                :focusable="false"
                @click="goToChannelArchive"
              >
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
            <div v-if="hasToolUpdate(tool.key)" class="tool-field">
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
