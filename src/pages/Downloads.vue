<script setup lang="ts">
import { NCheckbox, NFlex, NLog } from "naive-ui";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { invoke } from "@tauri-apps/api/core";
import { useDownloadStore } from "@/stores/download";
import type { DownloadTask } from "@/types";

const downloadStore = useDownloadStore();

// ========== 分组逻辑 ==========

const activeTasks = computed(() =>
  downloadStore.tasks.filter(
    (t) => t.status === "downloading" || t.status === "paused",
  ),
);

const finishedTasks = computed(() =>
  downloadStore.tasks.filter(
    (t) =>
      t.status === "completed" ||
      t.status === "error" ||
      t.status === "cancelled",
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

  if (diff === 0) return "今天";
  if (diff === dayMs) return "昨天";
  if (diff === dayMs * 2) return "前天";
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

// ========== 日志展开 ==========
const expandedLogs = reactive(new Set<string>());

const toggleLog = (id: string) => {
  if (expandedLogs.has(id)) {
    expandedLogs.delete(id);
  } else {
    expandedLogs.add(id);
  }
};

const logContent = (task: DownloadTask) => {
  return task.logs.join("\n") || "暂无日志...";
};

// ========== 进度条状态 ==========
type ProgressStatus = "default" | "success" | "error" | "warning";
const progressStatus = (task: DownloadTask): ProgressStatus => {
  switch (task.status) {
    case "completed":
      return "success";
    case "error":
      return "error";
    case "paused":
      return "warning";
    default:
      return "default";
  }
};

// ========== 状态标签 ==========
const statusLabel = (task: DownloadTask) => {
  switch (task.status) {
    case "downloading":
      return task.speed || "下载中...";
    case "paused":
      return "已暂停";
    case "completed":
      return "下载完成";
    case "error":
      return "下载失败";
    case "cancelled":
      return "已取消";
    default:
      return "";
  }
};

const statusType = (
  task: DownloadTask,
): "default" | "success" | "error" | "warning" | "info" => {
  switch (task.status) {
    case "completed":
      return "success";
    case "error":
      return "error";
    case "paused":
      return "warning";
    case "downloading":
      return "info";
    default:
      return "default";
  }
};

// ========== 进度文本 ==========
const sizeProgress = (task: DownloadTask) => {
  if (!task.downloaded && !task.total) return "";
  if (task.downloaded && task.total)
    return `${task.downloaded} / ${task.total}`;
  if (task.total) return task.total;
  return "";
};

// ========== 缩略图 ==========
const coverErrors = reactive(new Set<string>());

// ========== 打开文件夹 ==========
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
        title: "文件不存在",
        content: "该文件已被删除或移动，是否从列表中移除该记录？",
        positiveText: "移除",
        negativeText: "取消",
        onPositiveClick: () => {
          downloadStore.removeTask(task.id);
        },
      });
      return;
    }
    await revealItemInDir(task.params.downloadDir);
  } catch (e: unknown) {
    window.$message.error(
      e instanceof Error ? e.message : String(e) || "打开文件夹失败",
    );
  }
};

// ========== 操作 ==========
const handlePause = (id: string) => {
  window.$dialog.warning({
    title: "暂停下载",
    content: "确定要暂停当前下载吗？",
    positiveText: "暂停",
    negativeText: "取消",
    onPositiveClick: async () => {
      try {
        await downloadStore.pauseTask(id);
      } catch (e: unknown) {
        window.$message.error(
          e instanceof Error ? e.message : String(e) || "暂停失败",
        );
      }
    },
  });
};

const handleResume = async (id: string) => {
  try {
    await downloadStore.resumeTask(id);
  } catch (e: unknown) {
    window.$message.error(
      e instanceof Error ? e.message : String(e) || "继续失败",
    );
  }
};

const handleCancel = (id: string) => {
  window.$dialog.error({
    title: "取消并删除",
    content: "确定要取消下载并删除已下载的文件吗？此操作不可恢复。",
    positiveText: "取消并删除",
    negativeText: "返回",
    onPositiveClick: async () => {
      try {
        await downloadStore.cancelTask(id);
      } catch (e: unknown) {
        window.$message.error(
          e instanceof Error ? e.message : String(e) || "取消失败",
        );
      }
    },
  });
};

const handleRetry = async (id: string) => {
  try {
    await downloadStore.retryTask(id);
  } catch (e: unknown) {
    window.$message.error(
      e instanceof Error ? e.message : String(e) || "重新下载失败",
    );
  }
};

const deleteFileChecked = ref(false);

const handleRemove = (task: DownloadTask) => {
  deleteFileChecked.value = false;
  const hasFile = task.status === "completed" && !!task.outputFile;
  window.$dialog.warning({
    title: "移除任务",
    content: () =>
      h(NFlex, { vertical: true, size: 12 }, () => [
        "确定要从列表中移除该任务吗？",
        hasFile
          ? h(
              NCheckbox,
              {
                checked: deleteFileChecked.value,
                "onUpdate:checked": (v: boolean) => {
                  deleteFileChecked.value = v;
                },
              },
              { default: () => "同时删除已下载的文件" },
            )
          : null,
      ]),
    positiveText: "移除",
    negativeText: "取消",
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
  window.$dialog.warning({
    title: "清空已完成",
    content: "确定要清空所有已完成/失败/已取消的任务吗？",
    positiveText: "清空",
    negativeText: "取消",
    onPositiveClick: () => {
      downloadStore.clearFinished();
    },
  });
};
</script>

<template>
  <n-flex vertical :size="24">
    <!-- ====== 下载中区域 ====== -->
    <div class="section">
      <n-flex align="center" :size="8" style="margin-bottom: 12px">
        <n-icon size="16"><icon-mdi-download /></n-icon>
        <n-text strong>下载中</n-text>
        <n-tag
          v-if="activeTasks.length > 0"
          size="small"
          round
          :bordered="false"
          type="info"
        >
          {{ activeTasks.length }}
        </n-tag>
      </n-flex>

      <div v-if="activeTasks.length === 0" class="section-empty">
        <n-empty description="暂无正在下载的任务" size="small" />
      </div>
      <template v-else>
        <div
          v-for="group in activeGroups"
          :key="'a-' + group.label"
          class="date-group"
        >
          <n-text depth="3" class="date-label">{{ group.label }}</n-text>
          <n-flex vertical :size="10">
            <n-card
              v-for="task in group.tasks"
              :key="task.id"
              size="small"
              class="task-card"
            >
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
                    <n-ellipsis
                      :line-clamp="1"
                      :tooltip="false"
                      class="task-title"
                    >
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
                      <n-tag
                        size="small"
                        :bordered="false"
                        round
                        :type="statusType(task)"
                      >
                        {{ statusLabel(task) }}
                      </n-tag>
                      <n-text v-if="sizeProgress(task)" depth="3">
                        {{ sizeProgress(task) }}
                      </n-text>
                      <n-text depth="3">{{ task.percent.toFixed(1) }}%</n-text>
                      <n-text
                        v-if="task.eta && task.status === 'downloading'"
                        depth="3"
                      >
                        ETA {{ task.eta }}
                      </n-text>
                    </n-flex>
                    <n-flex align="center" size="small">
                      <template v-if="task.status === 'downloading'">
                        <n-button
                          size="tiny"
                          strong
                          secondary
                          @click="handlePause(task.id)"
                        >
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
                            <n-icon size="16"
                              ><icon-mdi-close-circle-outline
                            /></n-icon>
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
                            <n-icon size="16"
                              ><icon-mdi-close-circle-outline
                            /></n-icon>
                          </template>
                        </n-button>
                      </template>
                      <n-divider vertical style="margin: 0 2px" />
                      <n-button
                        size="tiny"
                        strong
                        secondary
                        @click="toggleLog(task.id)"
                      >
                        <template #icon>
                          <n-icon size="16">
                            <icon-mdi-chevron-up
                              v-if="expandedLogs.has(task.id)"
                            /><icon-mdi-text-long v-else />
                          </n-icon>
                        </template>
                      </n-button>
                    </n-flex>
                  </n-flex>
                  <n-collapse-transition :show="expandedLogs.has(task.id)">
                    <div class="task-log">
                      <n-log
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

    <!-- ====== 已完成区域 ====== -->
    <div class="section">
      <n-flex align="center" :size="8" style="margin-bottom: 12px">
        <n-icon size="16"><icon-mdi-check-circle-outline /></n-icon>
        <n-text strong>已完成</n-text>
        <n-tag
          v-if="finishedTasks.length > 0"
          size="small"
          round
          :bordered="false"
          type="success"
        >
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
          清空
        </n-button>
      </n-flex>

      <div v-if="finishedTasks.length === 0" class="section-empty">
        <n-empty description="暂无已完成的任务" size="small" />
      </div>
      <template v-else>
        <div
          v-for="group in finishedGroups"
          :key="'f-' + group.label"
          class="date-group"
        >
          <n-text depth="3" class="date-label">{{ group.label }}</n-text>
          <n-flex vertical :size="10">
            <n-card
              v-for="task in group.tasks"
              :key="task.id"
              size="small"
              class="task-card"
            >
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
                    <n-ellipsis
                      :line-clamp="1"
                      :tooltip="false"
                      class="task-title"
                    >
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
                      <n-tag
                        size="small"
                        :bordered="false"
                        round
                        :type="statusType(task)"
                      >
                        {{ statusLabel(task) }}
                      </n-tag>
                      <template v-if="task.status !== 'completed'">
                        <n-text v-if="sizeProgress(task)" depth="3">
                          {{ sizeProgress(task) }}
                        </n-text>
                        <n-text depth="3"
                          >{{ task.percent.toFixed(1) }}%</n-text
                        >
                      </template>
                    </n-flex>
                    <n-flex align="center" size="small">
                      <n-button
                        v-if="task.status === 'completed'"
                        size="tiny"
                        strong
                        secondary
                        type="primary"
                        @click="handleOpenFolder(task)"
                      >
                        <template #icon>
                          <n-icon size="16"
                            ><icon-mdi-folder-open-outline
                          /></n-icon>
                        </template>
                      </n-button>
                      <n-button
                        v-if="
                          task.status === 'error' || task.status === 'cancelled'
                        "
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
                        size="tiny"
                        strong
                        secondary
                        @click="toggleLog(task.id)"
                      >
                        <template #icon>
                          <n-icon size="16">
                            <icon-mdi-chevron-up
                              v-if="expandedLogs.has(task.id)"
                            /><icon-mdi-text-long v-else />
                          </n-icon>
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

// ========== 日期分组 ==========
.date-group {
  margin-bottom: 12px;
}

.date-label {
  display: block;
  font-size: 12px;
  margin-bottom: 8px;
}

// ========== 任务卡片 ==========
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
