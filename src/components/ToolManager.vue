<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useI18n } from "vue-i18n";
import { useSettingStore } from "@/stores/setting";
import type { ToolOperationProgress, ToolSource, ToolStatus } from "@/types";

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
  await applySources();
  await Promise.all(tools.value.map(refreshTool));
};

const handleSourceChange = async (tool: ToolDefinition, value: SelectableToolSource) => {
  setSource(tool.key, value);
  await applySources();
  await refreshTool(tool);
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

const stageLabel = (state: OperationState) => {
  const key = `settings.toolStage.${state.stage}`;
  return t(key);
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
      <n-button size="small" strong secondary class="tool-action" @click="refreshAll">
        <template #icon>
          <n-icon><icon-mdi-refresh /></n-icon>
        </template>
        {{ $t("common.refresh") }}
      </n-button>
    </template>

    <n-flex vertical :size="12">
      <section v-for="tool in tools" :key="tool.key" class="tool-row">
        <div class="tool-main">
          <div class="tool-heading">
            <n-text strong>{{ tool.title }}</n-text>
            <n-tag
              v-if="!checking[tool.key]"
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

          <div class="tool-meta">
            <n-text depth="3">{{ $t("settings.version") }}</n-text>
            <n-text code>{{ statuses[tool.key]?.version || "—" }}</n-text>
            <n-text depth="3">{{ $t("settings.path") }}</n-text>
            <n-ellipsis :line-clamp="1" :tooltip="{ width: 420 }" class="tool-path">
              {{ statuses[tool.key]?.path || "—" }}
            </n-ellipsis>
          </div>

          <n-collapse-transition :show="operations[tool.key].active">
            <div class="operation-progress">
              <div class="progress-label">
                <n-text depth="2">{{ stageLabel(operations[tool.key]) }}</n-text>
                <n-text v-if="operations[tool.key].percent != null" class="progress-number">
                  {{ Math.round(operations[tool.key].percent || 0) }}%
                </n-text>
              </div>
              <n-progress
                type="line"
                :percentage="Math.round(operations[tool.key].percent || 0)"
                :processing="operations[tool.key].percent == null"
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
            :disabled="operations[tool.key].active || statuses[tool.key]?.source === 'custom'"
            class="source-select"
            @update:value="(value: SelectableToolSource) => handleSourceChange(tool, value)"
          />
          <n-button
            v-if="!statuses[tool.key]?.installed && statuses[tool.key]?.source !== 'custom'"
            type="primary"
            strong
            secondary
            size="small"
            class="tool-action"
            :disabled="operations[tool.key].active"
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
            class="tool-action"
            :disabled="operations[tool.key].active"
            @click="handleUpdate(tool)"
          >
            {{ $t("settings.updateNow") }}
          </n-button>
          <n-tooltip v-else>
            <template #trigger>
              <n-button size="small" secondary disabled class="tool-action">
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
    </n-flex>
  </n-card>
</template>

<style scoped lang="scss">
.tool-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 16px;
  padding: 14px;
  border-radius: 12px;
  box-shadow:
    0 0 0 1px rgba(0, 0, 0, 0.06),
    0 1px 2px -1px rgba(0, 0, 0, 0.06),
    0 2px 4px rgba(0, 0, 0, 0.04);
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
  margin-top: 4px;
  font-size: 13px;
  text-wrap: pretty;
}

.tool-meta {
  display: grid;
  grid-template-columns: auto minmax(80px, auto) auto minmax(120px, 1fr);
  align-items: center;
  gap: 8px;
  margin-top: 10px;
  font-size: 12px;
}

.tool-path {
  min-width: 0;
}

.tool-controls {
  display: flex;
  align-items: flex-start;
  gap: 8px;
}

.source-select {
  width: 132px;
}

.tool-action {
  min-height: 40px;
  transition-property: scale;
  transition-duration: 150ms;
  transition-timing-function: ease-out;

  &:active:not(:disabled) {
    scale: 0.96;
  }
}

.operation-progress {
  margin-top: 12px;
}

.progress-label {
  margin-bottom: 5px;
  font-size: 12px;
}

.progress-number {
  font-variant-numeric: tabular-nums;
}

@media (max-width: 720px) {
  .tool-row {
    grid-template-columns: 1fr;
  }

  .tool-controls {
    flex-wrap: wrap;
  }

  .tool-meta {
    grid-template-columns: auto 1fr;
  }
}
</style>
