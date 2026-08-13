<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { useI18n } from "vue-i18n";
import { useSettingStore } from "@/stores/setting";
import type { ToolOperationProgress, ToolSource, ToolStatus, ToolUpdateCheck } from "@/types";

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
    await invoke(command);
    window.$message.success(
      operation === "install"
        ? t("settings.toolInstallComplete", { tool: tool.title })
        : t("settings.toolUpdateComplete", { tool: tool.title }),
    );
    await refreshTool(tool);
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

const handleCheckUpdate = async (tool: ToolDefinition) => {
  checkingUpdates[tool.key] = true;
  try {
    const result = await invoke<ToolUpdateCheck>("check_tool_update", { tool: tool.key });
    if (!result.updateAvailable) {
      window.$message.success(t("settings.alreadyLatest"));
      return;
    }
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
  } catch (e: unknown) {
    window.$message.error(t("settings.toolUpdateCheckFailed", { tool: tool.title, e }));
  } finally {
    checkingUpdates[tool.key] = false;
  }
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
          <div class="tool-main">
            <div class="tool-heading">
              <n-text strong>{{ tool.title }}</n-text>
              <n-tag
                v-if="statuses[tool.key]"
                size="small"
                round
                :type="statuses[tool.key]?.installed ? 'success' : 'error'"
              >
                {{
                  statuses[tool.key]?.installed
                    ? $t("settings.installed")
                    : $t("settings.notInstalled")
                }}
              </n-tag>
            </div>
            <n-text depth="3" class="tool-description">{{ tool.description }}</n-text>

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
          </div>

          <div class="tool-controls">
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
          </div>
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
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 16px;
}

.tool-main {
  min-width: 0;
}

.tool-heading,
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

.tool-controls {
  display: grid;
  grid-template-columns: 132px max-content;
  align-content: start;
  gap: 8px;
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

@media (max-width: 720px) {
  .tool-row-content {
    grid-template-columns: 1fr;
  }

  .tool-controls {
    justify-content: end;
  }
}
</style>
