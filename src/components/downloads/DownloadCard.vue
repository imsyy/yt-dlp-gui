<script setup lang="ts">
import { NFlex, NCheckbox, type LogInst } from "naive-ui";
import { revealItemInDir, openUrl } from "@tauri-apps/plugin-opener";
import { invoke } from "@tauri-apps/api/core";
import { useDownloadStore } from "@/stores/download";
import { useI18n } from "vue-i18n";
import { formatError } from "@/utils/format";
import { cleanTaskResiduals } from "@/utils/taskFiles";
import type { DownloadTask } from "@/types";

interface Props {
  task: DownloadTask;
}

const props = defineProps<Props>();

const { t } = useI18n();
const downloadStore = useDownloadStore();

const coverError = ref(false);
const isLogExpanded = ref(false);
const logRef = ref<LogInst | null>(null);

const isCompleted = computed(() => props.task.status === "completed");
const isActive = computed(
  () =>
    props.task.status === "preparing" ||
    props.task.status === "downloading" ||
    props.task.status === "postprocessing" ||
    props.task.status === "queued",
);
const isProcessing = computed(
  () =>
    props.task.status === "preparing" ||
    props.task.status === "downloading" ||
    props.task.status === "postprocessing",
);

type ProgressStatus = "default" | "success" | "error" | "warning";
const progressStatus = computed<ProgressStatus>(() => {
  switch (props.task.status) {
    case "completed":
      return "success";
    case "error":
      return "error";
    case "queued":
    case "postprocessing":
      return "warning";
    default:
      return "default";
  }
});

const statusLabel = computed<string>(() => {
  switch (props.task.status) {
    case "preparing":
      return t("downloads.status.preparing");
    case "queued":
      return t("downloads.status.queued");
    case "downloading":
      return t("downloads.status.downloading");
    case "postprocessing":
      return t("downloads.status.postprocessing");
    case "completed":
      return t("downloads.status.completed");
    case "error":
      return t("downloads.status.error");
    case "cancelled":
      return t("downloads.status.cancelled");
    default:
      return "";
  }
});

const statusType = computed<"default" | "success" | "error" | "warning" | "info">(() => {
  switch (props.task.status) {
    case "preparing":
      return "info";
    case "completed":
      return "success";
    case "error":
      return "error";
    case "queued":
      return "warning";
    case "downloading":
    case "postprocessing":
      return "info";
    default:
      return "default";
  }
});

const sizeProgress = computed<string>(() => {
  if (isCompleted.value) {
    return props.task.total || "";
  }
  if (!props.task.downloaded && !props.task.total) return "";
  if (props.task.downloaded && props.task.total) {
    return `${props.task.downloaded} / ${props.task.total}`;
  }
  if (props.task.total) return props.task.total;
  return "";
});

const logContent = computed<string>(() => {
  return props.task.logs.join("\n") || t("downloads.noLogs");
});

const toggleLog = () => {
  isLogExpanded.value = !isLogExpanded.value;
  if (isLogExpanded.value) {
    nextTick(() => {
      logRef.value?.scrollTo({ position: "bottom", silent: true });
    });
  }
};

watch(
  () => props.task.logs.length,
  () => {
    if (isLogExpanded.value) {
      nextTick(() => {
        logRef.value?.scrollTo({ position: "bottom", silent: true });
      });
    }
  },
);

/** 打开源网页 */
const handleOpenSource = async () => {
  try {
    await openUrl(props.task.url);
  } catch (error: unknown) {
    window.$message.error(error instanceof Error ? error.message : String(error));
  }
};

/** 打开文件所在本地文件夹（仅已完成） */
const handleOpenFolder = async () => {
  try {
    if (props.task.outputFile) {
      const [exists] = await invoke<boolean[]>("check_files_exist", {
        paths: [props.task.outputFile],
      });
      if (exists) {
        await revealItemInDir(props.task.outputFile);
        return;
      }
      window.$dialog.warning({
        title: t("downloads.fileNotExist"),
        content: t("downloads.fileDeletedOrMoved"),
        positiveText: t("common.remove"),
        negativeText: t("common.cancel"),
        onPositiveClick: () => {
          downloadStore.removeTask(props.task.id);
        },
      });
      return;
    }
    await revealItemInDir(props.task.params.downloadDir);
  } catch (error: unknown) {
    window.$message.error(
      error instanceof Error ? error.message : String(error) || t("downloads.openFolderFailed"),
    );
  }
};

/** 取消下载任务（仅进行中） */
const handleCancel = () => {
  window.$dialog.warning({
    title: t("downloads.cancelDownload"),
    content: t("downloads.confirmCancelDownload"),
    positiveText: t("downloads.cancelDownload"),
    negativeText: t("common.back"),
    onPositiveClick: async () => {
      try {
        await downloadStore.cancelTask(props.task.id);
      } catch (error: unknown) {
        window.$message.error(
          error instanceof Error ? error.message : String(error) || t("downloads.cancelFailed"),
        );
      }
    },
  });
};

/** 重新下载已完成的任务（弹窗二次确认，克隆为全新独立任务入队） */
const handleReDownload = () => {
  window.$dialog.info({
    title: t("downloads.reDownload"),
    content: t("downloads.confirmReDownload"),
    positiveText: t("downloads.reDownload"),
    negativeText: t("common.cancel"),
    onPositiveClick: async () => {
      try {
        await downloadStore.reDownloadTask(props.task.id);
      } catch (error: unknown) {
        window.$message.error(
          error instanceof Error ? error.message : String(error) || t("downloads.retryFailed"),
        );
      }
    },
  });
};

/** 原位重试失败或取消的任务（弹窗二次确认） */
const handleRetry = () => {
  window.$dialog.info({
    title: t("downloads.reDownload"),
    content: t("downloads.confirmRetry"),
    positiveText: t("downloads.reDownload"),
    negativeText: t("common.cancel"),
    onPositiveClick: async () => {
      try {
        await downloadStore.retryTask(props.task.id);
      } catch (error: unknown) {
        window.$message.error(
          error instanceof Error ? error.message : String(error) || t("downloads.retryFailed"),
        );
      }
    },
  });
};

const deleteFileChecked = ref(false);

/** 从列表中移除任务记录（已完成可删除输出文件，失败或已取消可清理残留临时文件） */
const handleRemove = () => {
  deleteFileChecked.value = false;
  const isFinished = isCompleted.value;
  const hasOutputFile = isFinished && !!props.task.outputFile;
  const isInterrupted = props.task.status === "cancelled" || props.task.status === "error";

  // 失败或已取消任务默认勾选清理残留文件
  if (isInterrupted) {
    deleteFileChecked.value = true;
  }

  window.$dialog.warning({
    title: t("downloads.removeTask"),
    content: () =>
      h(NFlex, { vertical: true, size: 12 }, () => [
        t("downloads.confirmRemoveTask"),
        hasOutputFile
          ? h(
              NCheckbox,
              {
                checked: deleteFileChecked.value,
                "onUpdate:checked": (value: boolean) => {
                  deleteFileChecked.value = value;
                },
              },
              { default: () => t("downloads.alsoDeleteFiles") },
            )
          : isInterrupted
            ? h(
                NCheckbox,
                {
                  checked: deleteFileChecked.value,
                  "onUpdate:checked": (value: boolean) => {
                    deleteFileChecked.value = value;
                  },
                },
                { default: () => t("downloads.alsoDeleteResidualFiles") },
              )
            : null,
      ]),
    positiveText: t("common.remove"),
    negativeText: t("common.cancel"),
    onPositiveClick: async () => {
      if (hasOutputFile && deleteFileChecked.value && props.task.outputFile) {
        try {
          await invoke("delete_file", { path: props.task.outputFile });
        } catch {
          // 忽略文件删除失败
        }
      } else if (isInterrupted && deleteFileChecked.value) {
        try {
          await cleanTaskResiduals(props.task);
        } catch {
          // 忽略清理失败
        }
      }
      downloadStore.removeTask(props.task.id);
    },
  });
};
</script>

<template>
  <n-card size="small" class="task-card">
    <n-flex :size="14">
      <!-- 视频缩略图 -->
      <div class="task-thumbnail">
        <img
          v-if="task.thumbnail && !coverError"
          :src="task.thumbnail"
          @error="coverError = true"
        />
        <div v-else class="thumbnail-placeholder">
          <icon-mdi-video-outline />
        </div>
      </div>

      <!-- 任务详情与操作区域 -->
      <n-flex justify="between" vertical class="task-info">
        <n-flex align="center" :size="8" class="task-header">
          <n-tag size="small" :bordered="false" round :type="statusType">
            {{ task.formatLabel }}
          </n-tag>
          <n-ellipsis :line-clamp="1" :tooltip="false" class="task-title">
            {{ task.title }}
          </n-ellipsis>
        </n-flex>

        <n-progress
          :percentage="isCompleted ? 100 : task.percent"
          :show-indicator="false"
          :status="progressStatus"
          :processing="isProcessing"
          style="width: 100%"
        />
        <n-flex align="center" justify="space-between">
          <n-flex align="center" :size="8">
            <n-popover
              v-if="task.status === 'error' && task.error"
              trigger="click"
              placement="top-start"
              style="max-width: 460px"
            >
              <template #trigger>
                <n-tag size="small" :bordered="false" round type="error">
                  <template #icon>
                    <n-icon size="12"><icon-mdi-information-outline /></n-icon>
                  </template>
                  {{ statusLabel }}
                </n-tag>
              </template>
              <div class="error-popover-content">
                <div class="error-popover-title">
                  <n-text strong type="error">{{ $t("downloads.errorMessage") }}</n-text>
                </div>
                <div class="error-popover-body">
                  {{ formatError(task.error) }}
                </div>
              </div>
            </n-popover>

            <!-- 非错误状态 -->
            <n-tag v-else size="small" :bordered="false" round :type="statusType">
              {{ statusLabel }}
            </n-tag>

            <!-- 文件大小 -->
            <n-text v-if="sizeProgress" depth="3" class="task-stat">
              {{ sizeProgress }}
            </n-text>

            <!-- 下载实时速度 -->
            <n-text v-if="task.speed && task.status === 'downloading'" depth="3" class="task-stat">
              {{ task.speed }}
            </n-text>

            <!-- 进度百分比 -->
            <n-text v-if="task.percent > 0 && !isCompleted" depth="3" class="task-stat">
              {{ task.percent.toFixed(1) }}%
            </n-text>

            <!-- 预计剩余时间 (ETA) -->
            <n-text v-if="task.eta && task.status === 'downloading'" depth="3" class="task-stat">
              ETA {{ task.eta }}
            </n-text>
          </n-flex>

          <!-- 右侧操作按钮组 -->
          <n-flex align="center" size="small">
            <!-- 打开源网页链接 -->
            <n-tooltip trigger="hover">
              <template #trigger>
                <n-button size="tiny" strong secondary @click="handleOpenSource">
                  <template #icon>
                    <n-icon size="16"><icon-mdi-open-in-new /></n-icon>
                  </template>
                </n-button>
              </template>
              {{ $t("common.open") }}
            </n-tooltip>

            <!-- 查看/收起任务日志（进行中或失败状态） -->
            <n-button
              v-if="isActive || task.status === 'error'"
              size="tiny"
              strong
              secondary
              @click="toggleLog"
            >
              <template #icon>
                <n-icon size="16">
                  <icon-mdi-chevron-up v-if="isLogExpanded" />
                  <icon-mdi-text-long v-else />
                </n-icon>
              </template>
            </n-button>

            <!-- 重新下载按钮：已完成状态 -->
            <n-tooltip v-if="isCompleted" trigger="hover">
              <template #trigger>
                <n-button size="tiny" strong secondary type="primary" @click="handleReDownload">
                  <template #icon>
                    <n-icon size="16"><icon-mdi-refresh /></n-icon>
                  </template>
                </n-button>
              </template>
              {{ $t("downloads.reDownload") }}
            </n-tooltip>

            <!-- 重试下载按钮 -->
            <n-tooltip
              v-if="task.status === 'error' || task.status === 'cancelled'"
              trigger="hover"
            >
              <template #trigger>
                <n-button size="tiny" strong secondary type="primary" @click="handleRetry">
                  <template #icon>
                    <n-icon size="16"><icon-mdi-refresh /></n-icon>
                  </template>
                </n-button>
              </template>
              {{ $t("downloads.reDownload") }}
            </n-tooltip>

            <!-- 定位打开所在文件夹（仅已完成） -->
            <n-tooltip v-if="isCompleted" trigger="hover">
              <template #trigger>
                <n-button size="tiny" strong secondary @click="handleOpenFolder">
                  <template #icon>
                    <n-icon size="16"><icon-mdi-folder-open-outline /></n-icon>
                  </template>
                </n-button>
              </template>
              {{ $t("common.openFolder") }}
            </n-tooltip>

            <n-divider vertical style="margin: 0 2px" />

            <!-- 取消任务按钮 -->
            <n-tooltip v-if="isActive" trigger="hover">
              <template #trigger>
                <n-button size="tiny" strong secondary type="error" @click="handleCancel">
                  <template #icon>
                    <n-icon size="16"><icon-mdi-close-circle-outline /></n-icon>
                  </template>
                </n-button>
              </template>
              {{ $t("downloads.cancelDownload") }}
            </n-tooltip>

            <!-- 移除记录按钮 -->
            <n-tooltip v-else trigger="hover">
              <template #trigger>
                <n-button type="error" size="tiny" strong secondary @click="handleRemove">
                  <template #icon>
                    <n-icon size="16"><icon-mdi-delete-outline /></n-icon>
                  </template>
                </n-button>
              </template>
              {{ $t("downloads.removeTask") }}
            </n-tooltip>
          </n-flex>
        </n-flex>

        <n-collapse-transition :show="isLogExpanded">
          <div class="task-log">
            <n-log ref="logRef" :log="logContent" :rows="8" :font-size="12" :trim="false" />
          </div>
        </n-collapse-transition>
      </n-flex>
    </n-flex>
  </n-card>
</template>

<style scoped lang="scss">
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

.task-stat {
  font-variant-numeric: tabular-nums;
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

.error-popover-content {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 2px 4px;
}

.error-popover-title {
  font-size: 13px;
}

.error-popover-body {
  font-size: 12px;
  line-height: 1.5;
  color: var(--n-text-color-2);
  word-break: break-all;
  max-height: 240px;
  overflow-y: auto;
  user-select: text;
}

.task-log {
  border-radius: 8px;
  padding: 6px 0 6px 6px;
  overflow: hidden;
  background: var(--n-color-modal);
}
</style>
