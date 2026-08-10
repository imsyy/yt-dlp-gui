<script setup lang="ts">
import { NCheckbox, NFlex, NLog, type LogInst } from "naive-ui";
import { revealItemInDir, openUrl } from "@tauri-apps/plugin-opener";
import { invoke } from "@tauri-apps/api/core";
import { useDownloadStore } from "@/stores/download";
import { useSettingStore } from "@/stores/setting";
import { useI18n } from "vue-i18n";
import type { DownloadTask } from "@/types";

const { t } = useI18n();
const downloadStore = useDownloadStore();
const settingStore = useSettingStore();

const activeTasks = computed(() =>
  downloadStore.tasks.filter(
    (t) => t.status === "downloading" || t.status === "paused" || t.status === "queued",
  ),
);

const finishedTasks = computed(() =>
  downloadStore.tasks.filter(
    (t) => t.status === "completed" || t.status === "error" || t.status === "cancelled",
  ),
);

interface DateGroup {
  label: string;
  tasks: DownloadTask[];
}

const formatDateLabel = (timestamp: number): string => {
  const date = new Date(timestamp);
  const now = new Date();
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const target = new Date(date.getFullYear(), date.getMonth(), date.getDate());
  const diff = today.getTime() - target.getTime();
  const dayMs = 86400000;

  if (diff === 0) return t("downloads.today");
  if (diff === dayMs) return t("downloads.yesterday");
  if (diff === dayMs * 2) return t("downloads.dayBeforeYesterday");
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
};

const groupByDate = (tasks: DownloadTask[]): DateGroup[] => {
  const sorted = [...tasks].sort((a, b) => b.createdAt - a.createdAt);
  const map = new Map<string, DownloadTask[]>();
  for (const task of sorted) {
    const label = formatDateLabel(task.createdAt);
    if (!map.has(label)) map.set(label, []);
    map.get(label)!.push(task);
  }
  return Array.from(map.entries()).map(([label, tasks]) => ({ label, tasks }));
};

const activeGroups = computed(() => groupByDate(activeTasks.value));
const finishedGroups = computed(() => groupByDate(finishedTasks.value));

const expandedLogs = reactive(new Set<string>());

const toggleLog = (id: string) => {
  if (expandedLogs.has(id)) {
    expandedLogs.delete(id);
  } else {
    expandedLogs.add(id);
  }
};

const logContent = (task: DownloadTask) => {
  return task.logs.join("\n") || t("downloads.noLogs");
};

// 日志自动滚动到底部
const logRefs = new Map<string, LogInst>();
const setLogRef = (id: string) => (el: unknown) => {
  if (el) logRefs.set(id, el as LogInst);
  else logRefs.delete(id);
};

watch(
  () =>
    [...expandedLogs].map((id) => {
      const task = downloadStore.tasks.find((t) => t.id === id);
      return task ? task.logs.length : 0;
    }),
  () => {
    nextTick(() => {
      for (const id of expandedLogs) {
        logRefs.get(id)?.scrollTo({ position: "bottom", silent: true });
      }
    });
  },
);

type ProgressStatus = "default" | "success" | "error" | "warning";
const progressStatus = (task: DownloadTask): ProgressStatus => {
  switch (task.status) {
    case "completed":
      return "success";
    case "error":
      return "error";
    case "paused":
    case "queued":
      return "warning";
    default:
      return "default";
  }
};

const statusLabel = (task: DownloadTask) => {
  switch (task.status) {
    case "queued":
      return t("downloads.status.queued");
    case "downloading":
      return task.speed || t("downloads.status.downloading");
    case "paused":
      return t("downloads.status.paused");
    case "completed":
      return t("downloads.status.completed");
    case "error":
      return t("downloads.status.error");
    case "cancelled":
      return t("downloads.status.cancelled");
    default:
      return "";
  }
};

const statusType = (task: DownloadTask): "default" | "success" | "error" | "warning" | "info" => {
  switch (task.status) {
    case "completed":
      return "success";
    case "error":
      return "error";
    case "paused":
    case "queued":
      return "warning";
    case "downloading":
      return "info";
    default:
      return "default";
  }
};

const sizeProgress = (task: DownloadTask) => {
  if (!task.downloaded && !task.total) return "";
  if (task.downloaded && task.total) return `${task.downloaded} / ${task.total}`;
  if (task.total) return task.total;
  return "";
};

const coverErrors = reactive(new Set<string>());

const handleOpenFolder = async (task: DownloadTask) => {
  try {
    if (task.outputFile) {
      const [exists] = await invoke<boolean[]>("check_files_exist", {
        paths: [task.outputFile],
      });
      if (exists) {
        await revealItemInDir(task.outputFile);
        return;
      }
      window.$dialog.warning({
        title: t("downloads.fileNotExist"),
        content: t("downloads.fileDeletedOrMoved"),
        positiveText: t("common.remove"),
        negativeText: t("common.cancel"),
        onPositiveClick: () => {
          downloadStore.removeTask(task.id);
        },
      });
      return;
    }
    await revealItemInDir(task.params.downloadDir);
  } catch (e: unknown) {
    window.$message.error(
      e instanceof Error ? e.message : String(e) || t("downloads.openFolderFailed"),
    );
  }
};

const handleOpenSource = async (url: string) => {
  try {
    await openUrl(url);
  } catch (e: unknown) {
    window.$message.error(e instanceof Error ? e.message : String(e));
  }
};

const handlePause = async (id: string) => {
  try {
    await downloadStore.pauseTask(id);
  } catch (e: unknown) {
    window.$message.error(e instanceof Error ? e.message : String(e) || t("downloads.pauseFailed"));
  }
};

const handleResume = async (id: string) => {
  try {
    await downloadStore.resumeTask(id);
  } catch (e: unknown) {
    window.$message.error(
      e instanceof Error ? e.message : String(e) || t("downloads.resumeFailed"),
    );
  }
};

const handleCancel = (id: string) => {
  window.$dialog.error({
    title: t("downloads.cancelAndDelete"),
    content: t("downloads.confirmCancelAndDelete"),
    positiveText: t("downloads.cancelAndDelete"),
    negativeText: t("common.back"),
    onPositiveClick: async () => {
      try {
        await downloadStore.cancelTask(id);
      } catch (e: unknown) {
        window.$message.error(
          e instanceof Error ? e.message : String(e) || t("downloads.cancelFailed"),
        );
      }
    },
  });
};

const handleRetry = async (id: string) => {
  try {
    await downloadStore.retryTask(id);
  } catch (e: unknown) {
    window.$message.error(e instanceof Error ? e.message : String(e) || t("downloads.retryFailed"));
  }
};

const deleteFileChecked = ref(false);

const handleRemove = (task: DownloadTask) => {
  deleteFileChecked.value = false;
  const hasFile = task.status === "completed" && !!task.outputFile;
  window.$dialog.warning({
    title: t("downloads.removeTask"),
    content: () =>
      h(NFlex, { vertical: true, size: 12 }, () => [
        t("downloads.confirmRemoveTask"),
        hasFile
          ? h(
              NCheckbox,
              {
                checked: deleteFileChecked.value,
                "onUpdate:checked": (v: boolean) => {
                  deleteFileChecked.value = v;
                },
              },
              { default: () => t("downloads.alsoDeleteFiles") },
            )
          : null,
      ]),
    positiveText: t("common.remove"),
    negativeText: t("common.cancel"),
    onPositiveClick: async () => {
      if (hasFile && deleteFileChecked.value && task.outputFile) {
        try {
          await invoke("delete_file", { path: task.outputFile });
        } catch {
          // 文件可能已不存在，忽略
        }
      }
      downloadStore.removeTask(task.id);
    },
  });
};

const handleClearFinished = () => {
  if (!settingStore.confirmClearCompleted) {
    downloadStore.clearFinished();
    return;
  }
  window.$dialog.warning({
    title: t("downloads.clearCompleted"),
    content: t("downloads.confirmClearCompleted"),
    positiveText: t("common.clear"),
    negativeText: t("common.cancel"),
    onPositiveClick: () => {
      downloadStore.clearFinished();
    },
  });
};
</script>

<template>
  <n-flex vertical :size="24">
    <div class="section">
      <n-flex align="center" :size="8" style="margin-bottom: 12px">
        <n-icon size="16"><icon-mdi-download /></n-icon>
        <n-text strong>{{ $t("downloads.downloading") }}</n-text>
        <n-tag v-if="activeTasks.length > 0" size="small" round :bordered="false" type="info">
          {{ activeTasks.length }}
        </n-tag>
      </n-flex>

      <div v-if="activeTasks.length === 0" class="section-empty">
        <n-empty :description="$t('downloads.noActiveTasks')" size="small" />
      </div>
      <template v-else>
        <div v-for="group in activeGroups" :key="'a-' + group.label" class="date-group">
          <n-text depth="3" class="date-label">{{ group.label }}</n-text>
          <n-flex vertical :size="10">
            <n-card v-for="task in group.tasks" :key="task.id" size="small" class="task-card">
              <n-flex :size="14">
                <div class="task-thumbnail">
                  <img
                    v-if="task.thumbnail && !coverErrors.has(task.id)"
                    :src="task.thumbnail"
                    @error="coverErrors.add(task.id)"
                  />
                  <div v-else class="thumbnail-placeholder">
                    <icon-mdi-video-outline />
                  </div>
                </div>
                <n-flex justify="between" vertical class="task-info">
                  <n-flex align="center" :size="8" class="task-header">
                    <n-tag size="small" :bordered="false" round type="info">
                      {{ task.formatLabel }}
                    </n-tag>
                    <n-ellipsis :line-clamp="1" :tooltip="false" class="task-title">
                      {{ task.title }}
                    </n-ellipsis>
                  </n-flex>
                  <n-progress
                    :percentage="task.percent"
                    :show-indicator="false"
                    :status="progressStatus(task)"
                    :processing="task.status === 'downloading'"
                    style="width: 100%"
                  />
                  <n-flex align="center" justify="space-between">
                    <n-flex align="center">
                      <n-tag size="small" :bordered="false" round :type="statusType(task)">
                        {{ statusLabel(task) }}
                      </n-tag>
                      <n-text v-if="sizeProgress(task)" depth="3">
                        {{ sizeProgress(task) }}
                      </n-text>
                      <n-text depth="3">{{ task.percent.toFixed(1) }}%</n-text>
                      <n-text v-if="task.eta && task.status === 'downloading'" depth="3">
                        ETA {{ task.eta }}
                      </n-text>
                    </n-flex>
                    <n-flex align="center" size="small">
                      <n-button size="tiny" strong secondary @click="handleOpenSource(task.url)">
                        <template #icon>
                          <n-icon size="16"><icon-mdi-open-in-new /></n-icon>
                        </template>
                      </n-button>
                      <n-button size="tiny" strong secondary @click="toggleLog(task.id)">
                        <template #icon>
                          <n-icon size="16">
                            <icon-mdi-chevron-up v-if="expandedLogs.has(task.id)" />
                            <icon-mdi-text-long v-else />
                          </n-icon>
                        </template>
                      </n-button>
                      <n-divider vertical style="margin: 0 2px" />
                      <template v-if="task.status === 'downloading'">
                        <n-button size="tiny" strong secondary @click="handlePause(task.id)">
                          <template #icon>
                            <n-icon size="16"><icon-mdi-pause /></n-icon>
                          </template>
                        </n-button>
                        <n-button
                          size="tiny"
                          strong
                          secondary
                          type="error"
                          @click="handleCancel(task.id)"
                        >
                          <template #icon>
                            <n-icon size="16"><icon-mdi-close-circle-outline /></n-icon>
                          </template>
                        </n-button>
                      </template>
                      <template v-else-if="task.status === 'queued'">
                        <n-button
                          size="tiny"
                          strong
                          secondary
                          type="error"
                          @click="handleCancel(task.id)"
                        >
                          <template #icon>
                            <n-icon size="16"><icon-mdi-close-circle-outline /></n-icon>
                          </template>
                        </n-button>
                      </template>
                      <template v-else-if="task.status === 'paused'">
                        <n-button
                          size="tiny"
                          strong
                          secondary
                          type="primary"
                          @click="handleResume(task.id)"
                        >
                          <template #icon>
                            <n-icon size="16"><icon-mdi-play /></n-icon>
                          </template>
                        </n-button>
                        <n-button
                          size="tiny"
                          strong
                          secondary
                          type="error"
                          @click="handleCancel(task.id)"
                        >
                          <template #icon>
                            <n-icon size="16"><icon-mdi-close-circle-outline /></n-icon>
                          </template>
                        </n-button>
                      </template>
                    </n-flex>
                  </n-flex>
                  <n-collapse-transition :show="expandedLogs.has(task.id)">
                    <div class="task-log">
                      <n-log
                        :ref="setLogRef(task.id)"
                        :log="logContent(task)"
                        :rows="8"
                        :font-size="12"
                        :trim="false"
                      />
                    </div>
                  </n-collapse-transition>
                </n-flex>
              </n-flex>
            </n-card>
          </n-flex>
        </div>
      </template>
    </div>

    <div class="section">
      <n-flex align="center" :size="8" style="margin-bottom: 12px">
        <n-icon size="16"><icon-mdi-check-circle-outline /></n-icon>
        <n-text strong>{{ $t("downloads.completed") }}</n-text>
        <n-tag v-if="finishedTasks.length > 0" size="small" round :bordered="false" type="success">
          {{ finishedTasks.length }}
        </n-tag>
        <n-button
          v-if="finishedTasks.length > 0"
          size="tiny"
          strong
          secondary
          type="error"
          style="margin-left: auto"
          @click="handleClearFinished"
        >
          <template #icon>
            <n-icon size="14"><icon-mdi-delete-sweep-outline /></n-icon>
          </template>
          {{ $t("common.clear") }}
        </n-button>
      </n-flex>

      <div v-if="finishedTasks.length === 0" class="section-empty">
        <n-empty :description="$t('downloads.noFinishedTasks')" size="small" />
      </div>
      <template v-else>
        <div v-for="group in finishedGroups" :key="'f-' + group.label" class="date-group">
          <n-text depth="3" class="date-label">{{ group.label }}</n-text>
          <n-flex vertical :size="10">
            <n-card v-for="task in group.tasks" :key="task.id" size="small" class="task-card">
              <n-flex :size="14">
                <div class="task-thumbnail">
                  <img
                    v-if="task.thumbnail && !coverErrors.has(task.id)"
                    :src="task.thumbnail"
                    @error="coverErrors.add(task.id)"
                  />
                  <div v-else class="thumbnail-placeholder">
                    <icon-mdi-video-outline />
                  </div>
                </div>
                <n-flex justify="between" vertical class="task-info">
                  <n-flex align="center" :size="8" class="task-header">
                    <n-tag size="small" :bordered="false" round type="info">
                      {{ task.formatLabel }}
                    </n-tag>
                    <n-ellipsis :line-clamp="1" :tooltip="false" class="task-title">
                      {{ task.title }}
                    </n-ellipsis>
                  </n-flex>
                  <n-progress
                    :percentage="task.percent"
                    :show-indicator="false"
                    :status="progressStatus(task)"
                    style="width: 100%"
                  />
                  <n-flex align="center" justify="space-between">
                    <n-flex align="center">
                      <n-tag size="small" :bordered="false" round :type="statusType(task)">
                        {{ statusLabel(task) }}
                      </n-tag>
                      <template v-if="task.status !== 'completed'">
                        <n-text v-if="sizeProgress(task)" depth="3">
                          {{ sizeProgress(task) }}
                        </n-text>
                        <n-text depth="3">{{ task.percent.toFixed(1) }}%</n-text>
                      </template>
                    </n-flex>
                    <n-flex align="center" size="small">
                      <n-button size="tiny" strong secondary @click="handleOpenSource(task.url)">
                        <template #icon>
                          <n-icon size="16"><icon-mdi-open-in-new /></n-icon>
                        </template>
                      </n-button>
                      <n-button size="tiny" strong secondary @click="toggleLog(task.id)">
                        <template #icon>
                          <n-icon size="16">
                            <icon-mdi-chevron-up v-if="expandedLogs.has(task.id)" />
                            <icon-mdi-text-long v-else />
                          </n-icon>
                        </template>
                      </n-button>
                      <n-divider vertical style="margin: 0 2px" />
                      <n-button
                        v-if="task.status === 'completed'"
                        size="tiny"
                        strong
                        secondary
                        type="primary"
                        @click="handleOpenFolder(task)"
                      >
                        <template #icon>
                          <n-icon size="16"><icon-mdi-folder-open-outline /></n-icon>
                        </template>
                      </n-button>
                      <n-button
                        v-if="task.status === 'error' || task.status === 'cancelled'"
                        size="tiny"
                        strong
                        secondary
                        type="primary"
                        @click="handleRetry(task.id)"
                      >
                        <template #icon>
                          <n-icon size="16"><icon-mdi-refresh /></n-icon>
                        </template>
                      </n-button>
                      <n-button
                        type="error"
                        size="tiny"
                        strong
                        secondary
                        @click="handleRemove(task)"
                      >
                        <template #icon>
                          <n-icon size="16"><icon-mdi-delete-outline /></n-icon>
                        </template>
                      </n-button>
                    </n-flex>
                  </n-flex>
                  <n-collapse-transition :show="expandedLogs.has(task.id)">
                    <div class="task-log">
                      <n-log
                        :ref="setLogRef(task.id)"
                        :log="logContent(task)"
                        :rows="8"
                        :font-size="12"
                        :trim="false"
                      />
                    </div>
                  </n-collapse-transition>
                </n-flex>
              </n-flex>
            </n-card>
          </n-flex>
        </div>
      </template>
    </div>
  </n-flex>
</template>

<style scoped lang="scss">
.section-empty {
  padding: 24px 0;
}

.date-group {
  margin-bottom: 12px;
}

.date-label {
  display: block;
  font-size: 12px;
  margin-bottom: 8px;
}

.task-card {
  :deep(.n-card__content) {
    padding: 14px;
  }
}

.task-thumbnail {
  flex-shrink: 0;
  width: 120px;
  height: 68px;
  border-radius: 6px;
  overflow: hidden;

  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .thumbnail-placeholder {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--n-color-modal);
    font-size: 28px;
    opacity: 0.4;
  }
}

.task-info {
  flex: 1;
  min-width: 0;
}

.task-header {
  min-width: 0;

  .task-title {
    flex: 1;
    min-width: 0;
    font-size: 14px;
    font-weight: 600;
    line-height: 1.4;
  }
}

.task-log {
  border-radius: 8px;
  padding: 6px 0 6px 6px;
  overflow: hidden;
  background: var(--n-color-modal);
}
</style>
