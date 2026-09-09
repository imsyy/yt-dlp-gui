<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { useI18n } from "vue-i18n";
import { useSettingStore } from "@/stores/setting";
import { useStatusStore } from "@/stores/status";
import type {
  ToolOperationProgress,
  ToolSource,
  ToolStatus,
  ToolUpdateCheck,
  YtdlpChannel,
} from "@/types";

type ToolKey = "yt-dlp" | "deno" | "ffmpeg";
type SelectableToolSource = Exclude<ToolSource, "custom">;

interface ToolDefinition {
  key: ToolKey;
  title: string;
  description: string;
  statusCommand: string;
  installCommand: string;
  updateCommand?: string;
}

interface OperationState {
  active: boolean;
  operation: "install" | "update";
  stage: ToolOperationProgress["stage"];
  percent: number | null;
}

const { t } = useI18n();
const settingStore = useSettingStore();
const statusStore = useStatusStore();

const tools = computed<ToolDefinition[]>(() => [
  {
    key: "yt-dlp",
    title: "yt-dlp",
    description: t("settings.ytdlpDesc"),
    statusCommand: "get_ytdlp_status",
    installCommand: "download_ytdlp",
    updateCommand: "update_ytdlp",
  },
  {
    key: "deno",
    title: t("settings.denoTitle"),
    description: t("settings.denoDesc"),
    statusCommand: "get_deno_status",
    installCommand: "download_deno",
    updateCommand: "update_deno",
  },
  {
    key: "ffmpeg",
    title: "FFmpeg + FFprobe",
    description: t("settings.ffmpegDesc"),
    statusCommand: "get_ffmpeg_status",
    installCommand: "download_ffmpeg",
    updateCommand: "update_ffmpeg",
  },
]);

const statuses = reactive<Record<ToolKey, ToolStatus | null>>({
  "yt-dlp": null,
  deno: null,
  ffmpeg: null,
});
const checking = reactive<Record<ToolKey, boolean>>({
  "yt-dlp": true,
  deno: true,
  ffmpeg: true,
});
const refreshing = ref(false);
const checkingUpdates = reactive<Record<ToolKey, boolean>>({
  "yt-dlp": false,
  deno: false,
  ffmpeg: false,
});
const operations = reactive<Record<ToolKey, OperationState>>({
  "yt-dlp": { active: false, operation: "install", stage: "downloading", percent: null },
  deno: { active: false, operation: "install", stage: "downloading", percent: null },
  ffmpeg: { active: false, operation: "install", stage: "downloading", percent: null },
});

const sourceOptions = computed(() => [
  { label: t("settings.sourceManaged"), value: "managed" },
  { label: t("settings.sourceSystem"), value: "system" },
]);

/** yt-dlp 发行通道：专有名词，无需翻译 */
const ytdlpChannelOptions: { label: string; value: YtdlpChannel }[] = [
  { label: "Stable", value: "stable" },
  { label: "Nightly", value: "nightly" },
  { label: "Master", value: "master" },
];

const getSource = (tool: ToolKey): SelectableToolSource => {
  if (tool === "yt-dlp") return settingStore.ytdlpSource;
  if (tool === "deno") return settingStore.denoSource;
  return settingStore.ffmpegSource;
};

const setSource = (tool: ToolKey, value: SelectableToolSource) => {
  if (tool === "yt-dlp") settingStore.ytdlpSource = value;
  else if (tool === "deno") settingStore.denoSource = value;
  else settingStore.ffmpegSource = value;
};

const applySources = () =>
  invoke("set_tool_sources", {
    ytdlp: settingStore.ytdlpSource,
    deno: settingStore.denoSource,
    ffmpeg: settingStore.ffmpegSource,
  });

/** 将前端持久化的 yt-dlp 通道同步到后端（后端通道状态随应用重启丢失，以前端为准） */
const applyYtdlpChannel = () => invoke("set_ytdlp_channel", { channel: settingStore.ytdlpChannel });

const refreshTool = async (tool: ToolDefinition) => {
  checking[tool.key] = true;
  try {
    statuses[tool.key] = await invoke<ToolStatus>(tool.statusCommand);
  } catch (e: unknown) {
    window.$message.error(t("settings.toolStatusFailed", { tool: tool.title, e }));
  } finally {
    checking[tool.key] = false;
  }
};

const refreshAll = async () => {
  refreshing.value = true;
  tools.value.forEach((tool) => {
    checking[tool.key] = true;
  });
  try {
    await applyYtdlpChannel();
    await applySources();
    await Promise.all(tools.value.map(refreshTool));
  } catch (e: unknown) {
    window.$message.error(t("settings.toolStatusFailed", { tool: t("settings.toolManager"), e }));
  } finally {
    tools.value.forEach((tool) => {
      checking[tool.key] = false;
    });
    refreshing.value = false;
  }
};

const handleSourceChange = async (tool: ToolDefinition, value: SelectableToolSource) => {
  checking[tool.key] = true;
  try {
    setSource(tool.key, value);
    await applySources();
    await refreshTool(tool);
  } catch (e: unknown) {
    window.$message.error(t("settings.toolStatusFailed", { tool: tool.title, e }));
  } finally {
    checking[tool.key] = false;
  }
};

const runOperation = async (
  tool: ToolDefinition,
  operation: "install" | "update",
  command: string,
) => {
  const state = operations[tool.key];
  state.active = true;
  state.operation = operation;
  state.stage = operation === "install" ? "downloading" : "updating";
  state.percent = null;
  try {
    // yt-dlp 下载/更新都使用当前所选通道的构建仓库，先同步再执行
    if (tool.key === "yt-dlp") await applyYtdlpChannel();
    await invoke(command);
    window.$message.success(
      operation === "install"
        ? t("settings.toolInstallComplete", { tool: tool.title })
        : t("settings.toolUpdateComplete", { tool: tool.title }),
    );
    await refreshTool(tool);
    if (tool.key === "yt-dlp" && statuses["yt-dlp"]?.isManaged) {
      // 本次安装/更新的是内置 yt-dlp，记录其构建通道，用于跨通道切换（含降级）判断
      settingStore.ytdlpInstalledChannel = settingStore.ytdlpChannel;
    }
    // 已更新到最新，清除之前检测到的更新标记
    statusStore.toolUpdates[tool.key] = null;
  } catch (e: unknown) {
    window.$message.error(t("settings.toolOperationFailed", { tool: tool.title, e }));
  } finally {
    state.active = false;
  }
};

const handleInstall = async (tool: ToolDefinition) => {
  if (getSource(tool.key) === "system") {
    setSource(tool.key, "managed");
    await applySources();
  }
  await runOperation(tool, "install", tool.installCommand);
};

const handleUpdate = (tool: ToolDefinition) => {
  const command = tool.updateCommand || tool.installCommand;
  return runOperation(tool, "update", command);
};

/** 通用的更新确认框：显示当前/最新版本，确认后更新 */
const showUpdateConfirmDialog = (tool: ToolDefinition, result: ToolUpdateCheck) => {
  window.$dialog.warning({
    title: t("settings.toolUpdateAvailable", { tool: tool.title }),
    content: t("settings.toolUpdateConfirm", {
      current: result.currentVersion,
      latest: result.latestVersion,
    }),
    positiveText: t("settings.updateNow"),
    negativeText: t("common.cancel"),
    onPositiveClick: () => {
      void handleUpdate(tool);
    },
  });
};

const handleCheckUpdate = async (tool: ToolDefinition) => {
  checkingUpdates[tool.key] = true;
  try {
    // yt-dlp 按当前所选通道检查对应仓库的最新构建，先同步通道再检查
    if (tool.key === "yt-dlp") await applyYtdlpChannel();
    const result = await invoke<ToolUpdateCheck>("check_tool_update", { tool: tool.key });
    // 记录检测结果：有更新时工具行标签切为"有更新"、状态栏亮点；无更新则清除旧标记
    statusStore.toolUpdates[tool.key] = result.updateAvailable ? result : null;
    if (!result.updateAvailable) {
      window.$message.success(t("settings.alreadyLatest"));
      return;
    }
    showUpdateConfirmDialog(tool, result);
  } catch (e: unknown) {
    window.$message.error(t("settings.toolUpdateCheckFailed", { tool: tool.title, e }));
  } finally {
    checkingUpdates[tool.key] = false;
  }
};

/** 切换 yt-dlp 发行通道；内置版本已安装时先检查新通道是否有更新，有才弹窗确认下载 */
const handleChannelChange = async (value: YtdlpChannel) => {
  const previous = settingStore.ytdlpChannel;
  if (value === previous) return;
  settingStore.ytdlpChannel = value;
  try {
    await applyYtdlpChannel();
  } catch (e: unknown) {
    settingStore.ytdlpChannel = previous;
    window.$message.error(t("settings.toolOperationFailed", { tool: "yt-dlp", e }));
    return;
  }
  const tool = tools.value.find((item) => item.key === "yt-dlp");
  if (!tool || statuses["yt-dlp"]?.source === "custom") return;
  if (!statuses["yt-dlp"]?.installed) return;
  // 仅内置版本需要跨通道切换处理（含降级）：版本比较会误判为"已是最新"，以通道切换为准。
  // 系统版本直接走常规检测，确认后用 yt-dlp --update-to 切换并更新。
  if (getSource("yt-dlp") !== "system" && settingStore.ytdlpInstalledChannel !== value) {
    await handleChannelSwitchDownload(tool);
    return;
  }
  await handleCheckUpdate(tool);
};

/** 跨通道切换（含降级）：版本相同则直接对齐记录，否则弹出通用的更新确认框 */
const handleChannelSwitchDownload = async (tool: ToolDefinition) => {
  checkingUpdates[tool.key] = true;
  try {
    const result = await invoke<ToolUpdateCheck>("check_tool_update", { tool: tool.key });
    if (result.currentVersion === result.latestVersion) {
      settingStore.ytdlpInstalledChannel = settingStore.ytdlpChannel;
      window.$message.success(t("settings.alreadyLatest"));
      return;
    }
    statusStore.toolUpdates[tool.key] = { ...result, updateAvailable: true };
    showUpdateConfirmDialog(tool, result);
  } catch (e: unknown) {
    window.$message.error(t("settings.toolUpdateCheckFailed", { tool: tool.title, e }));
  } finally {
    checkingUpdates[tool.key] = false;
  }
};

/** 该工具是否有检测到的可用更新 */
const hasToolUpdate = (tool: ToolKey) => statusStore.toolUpdates[tool]?.updateAvailable === true;

const toolTagType = (tool: ToolKey) => {
  if (hasToolUpdate(tool)) return "warning";
  return statuses[tool]?.installed ? "success" : "error";
};

const toolTagText = (tool: ToolKey) => {
  if (hasToolUpdate(tool)) return t("settings.updateAvailable");
  return statuses[tool]?.installed ? t("settings.installed") : t("settings.notInstalled");
};

const stageLabel = (state: OperationState) => {
  const key = `settings.toolStage.${state.stage}`;
  return t(key);
};

const progressPercentage = (state: OperationState) =>
  state.percent == null ? 100 : Math.round(state.percent);

const progressAriaValue = (state: OperationState) =>
  state.percent == null ? undefined : Math.round(state.percent);

const copyToolValue = async (label: string, value: string) => {
  if (!value) return;
  try {
    await writeText(value);
    window.$message.success(t("settings.toolValueCopied", { label }));
  } catch {
    window.$message.error(t("clipboard.writeFailed"));
  }
};

let unlistenProgress: (() => void) | null = null;

onMounted(async () => {
  unlistenProgress = await listen<ToolOperationProgress>("tool-operation-progress", (event) => {
    const payload = event.payload;
    const state = operations[payload.tool];
    state.active = payload.stage !== "complete";
    state.operation = payload.operation;
    state.stage = payload.stage;
    state.percent = payload.percent;
  });
  await refreshAll();
});

onUnmounted(() => unlistenProgress?.());
</script>

<template>
  <n-card :title="$t('settings.toolManager')" size="small" class="tool-manager section-card">
    <template #header-extra>
      <n-button size="small" strong secondary :loading="refreshing" @click="refreshAll">
        <template #icon>
          <n-icon><icon-mdi-refresh /></n-icon>
        </template>
        {{ $t("common.refresh") }}
      </n-button>
    </template>

    <div class="tool-list">
      <n-spin
        v-for="tool in tools"
        :key="tool.key"
        :show="checking[tool.key]"
        size="small"
        class="tool-row"
      >
        <section class="tool-row-content">
          <n-flex justify="space-between" align="center" class="tool-header">
            <div class="tool-heading">
              <n-text strong>{{ tool.title }}</n-text>
              <n-tag v-if="statuses[tool.key]" size="small" round :type="toolTagType(tool.key)">
                {{ toolTagText(tool.key) }}
              </n-tag>
            </div>
            <n-flex align="center" :size="8" :wrap="false" class="tool-controls">
              <n-select
                :value="getSource(tool.key)"
                :options="sourceOptions"
                size="small"
                :disabled="
                  checking[tool.key] ||
                  operations[tool.key].active ||
                  statuses[tool.key]?.source === 'custom'
                "
                class="source-select"
                @update:value="(value: SelectableToolSource) => handleSourceChange(tool, value)"
              />
              <n-button
                v-if="!statuses[tool.key]?.installed && statuses[tool.key]?.source !== 'custom'"
                type="primary"
                strong
                secondary
                size="small"
                :disabled="checking[tool.key] || operations[tool.key].active"
                @click="handleInstall(tool)"
              >
                {{
                  getSource(tool.key) === "system"
                    ? $t("settings.installManaged")
                    : $t("common.download")
                }}
              </n-button>
              <n-button
                v-else-if="statuses[tool.key]?.canUpdate"
                strong
                secondary
                size="small"
                :loading="checkingUpdates[tool.key]"
                :disabled="checking[tool.key] || operations[tool.key].active"
                @click="handleCheckUpdate(tool)"
              >
                {{ $t("settings.checkUpdate") }}
              </n-button>
              <n-tooltip v-else>
                <template #trigger>
                  <n-button size="small" secondary disabled>
                    {{
                      statuses[tool.key]?.source === "custom"
                        ? $t("settings.cliManaged")
                        : $t("settings.systemManaged")
                    }}
                  </n-button>
                </template>
                {{
                  statuses[tool.key]?.source === "custom"
                    ? $t("settings.cliManagedHint")
                    : $t("settings.systemManagedHint")
                }}
              </n-tooltip>
            </n-flex>
          </n-flex>
          <n-text depth="3" class="tool-description">{{ tool.description }}</n-text>

          <div v-if="tool.key === 'yt-dlp'" class="channel-row">
            <n-text depth="3" class="channel-label">
              {{ $t("settings.ytdlpChannel") }}
            </n-text>
            <n-select
              :value="settingStore.ytdlpChannel"
              :options="ytdlpChannelOptions"
              size="small"
              :disabled="
                checking[tool.key] ||
                operations[tool.key].active ||
                statuses[tool.key]?.source === 'custom'
              "
              class="channel-select"
              @update:value="(value: YtdlpChannel) => handleChannelChange(value)"
            />
          </div>

          <dl class="tool-meta">
            <div class="tool-meta-row">
              <dt>
                <n-text depth="3">{{ $t("settings.version") }}</n-text>
              </dt>
              <dd>
                <n-ellipsis :line-clamp="1" :tooltip="{ width: 480 }" class="tool-value">
                  <n-text strong>{{ statuses[tool.key]?.version || "—" }}</n-text>
                </n-ellipsis>
                <n-tooltip>
                  <template #trigger>
                    <n-button
                      quaternary
                      circle
                      size="tiny"
                      :disabled="!statuses[tool.key]?.version"
                      :aria-label="$t('common.copy')"
                      @click="
                        copyToolValue($t('settings.version'), statuses[tool.key]?.version || '')
                      "
                    >
                      <template #icon>
                        <n-icon><icon-mdi-content-copy /></n-icon>
                      </template>
                    </n-button>
                  </template>
                  {{ $t("common.copy") }}
                </n-tooltip>
                <n-text
                  v-if="hasToolUpdate(tool.key)"
                  type="warning"
                  class="latest-hint"
                  :title="$t('settings.latestVersion')"
                >
                  → {{ statusStore.toolUpdates[tool.key]?.latestVersion }}
                </n-text>
              </dd>
            </div>
            <div class="tool-meta-row">
              <dt>
                <n-text depth="3">{{ $t("settings.path") }}</n-text>
              </dt>
              <dd>
                <n-ellipsis :line-clamp="1" :tooltip="{ width: 560 }" class="tool-value">
                  {{ statuses[tool.key]?.path || "—" }}
                </n-ellipsis>
                <n-tooltip>
                  <template #trigger>
                    <n-button
                      quaternary
                      circle
                      size="tiny"
                      :disabled="!statuses[tool.key]?.path"
                      :aria-label="$t('common.copy')"
                      @click="copyToolValue($t('settings.path'), statuses[tool.key]?.path || '')"
                    >
                      <template #icon>
                        <n-icon><icon-mdi-content-copy /></n-icon>
                      </template>
                    </n-button>
                  </template>
                  {{ $t("common.copy") }}
                </n-tooltip>
              </dd>
            </div>
          </dl>

          <n-collapse-transition :show="operations[tool.key].active">
            <div class="operation-progress" aria-live="polite">
              <div class="progress-label">
                <n-text depth="2">{{ stageLabel(operations[tool.key]) }}</n-text>
                <n-text v-if="operations[tool.key].percent != null" class="progress-number">
                  {{ Math.round(operations[tool.key].percent || 0) }}%
                </n-text>
              </div>
              <n-progress
                type="line"
                :percentage="progressPercentage(operations[tool.key])"
                :processing="operations[tool.key].percent == null"
                :aria-valuenow="progressAriaValue(operations[tool.key])"
                :aria-valuetext="
                  operations[tool.key].percent == null
                    ? stageLabel(operations[tool.key])
                    : undefined
                "
                :show-indicator="false"
                :height="8"
                :border-radius="4"
              />
            </div>
          </n-collapse-transition>
        </section>
      </n-spin>
    </div>
  </n-card>
</template>

<style scoped lang="scss">
.tool-list {
  display: flex;
  flex-direction: column;
}

.tool-row {
  padding: 12px 0;

  &:first-child {
    padding-top: 0;
  }

  &:last-child {
    padding-bottom: 0;
  }

  & + & {
    border-top: 1px solid var(--n-border-color);
  }
}

.tool-row-content {
  display: flex;
  flex-direction: column;
}

.tool-heading {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.progress-label {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.tool-description {
  display: block;
  margin-top: 2px;
  font-size: 13px;
  text-wrap: pretty;
}

.channel-row {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 8px;
}

.channel-label {
  font-size: 12px;
  white-space: nowrap;
}

.channel-select {
  width: 220px;
}

.tool-meta {
  display: grid;
  grid-template-columns: max-content minmax(0, 1fr);
  gap: 4px 8px;
  margin: 8px 0 0;
  font-size: 12px;
}

.tool-meta-row {
  display: contents;

  dt {
    align-self: center;
    margin: 0;
    white-space: nowrap;
  }

  dd {
    display: flex;
    align-items: center;
    gap: 4px;
    min-width: 0;
    margin: 0;
  }
}

.tool-value {
  display: block;
  flex: 0 1 auto;
  min-width: 0;
}

.latest-hint {
  flex-shrink: 0;
  font-size: 12px;
  font-variant-numeric: tabular-nums;
}

.tool-controls {
  flex-shrink: 0;
}

.source-select {
  width: 132px;
}

.operation-progress {
  margin-top: 10px;
}

.progress-label {
  margin-bottom: 5px;
  font-size: 12px;
}

.progress-number {
  font-variant-numeric: tabular-nums;
}
</style>
