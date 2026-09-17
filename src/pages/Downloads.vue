<script setup lang="ts">
import { useDownloadStore } from "@/stores/download";
import { useI18n } from "vue-i18n";
import type { DownloadTask } from "@/types";

const { t } = useI18n();
const downloadStore = useDownloadStore();

/** 当前激活的分类标签页 */
type DownloadCategory = "all" | "downloading" | "completed" | "cancelled" | "failed";
const activeCategory = ref<DownloadCategory>("all");

/** 全部任务总数 */
const totalTasksCount = computed<number>(() => downloadStore.tasks.length);

/** 正在进行的任务（准备中、排队中、下载中、后处理中） */
const activeTasks = computed<DownloadTask[]>(() =>
  downloadStore.tasks.filter(
    (downloadTask) =>
      downloadTask.status === "preparing" ||
      downloadTask.status === "downloading" ||
      downloadTask.status === "postprocessing" ||
      downloadTask.status === "queued",
  ),
);

/** 进行中任务按创建时间倒序排列 */
const sortedActiveTasks = computed<DownloadTask[]>(() =>
  [...activeTasks.value].sort(
    (firstTask, secondTask) => secondTask.createdAt - firstTask.createdAt,
  ),
);

/** 仅已成功完成的任务 */
const completedTasks = computed<DownloadTask[]>(() =>
  downloadStore.tasks.filter((downloadTask) => downloadTask.status === "completed"),
);

/** 仅已主动取消的任务 */
const cancelledTasks = computed<DownloadTask[]>(() =>
  downloadStore.tasks.filter((downloadTask) => downloadTask.status === "cancelled"),
);

/** 取消任务按创建时间倒序排列 */
const sortedCancelledTasks = computed<DownloadTask[]>(() =>
  [...cancelledTasks.value].sort(
    (firstTask, secondTask) => secondTask.createdAt - firstTask.createdAt,
  ),
);

/** 仅下载失败/异常的任务 */
const failedTasks = computed<DownloadTask[]>(() =>
  downloadStore.tasks.filter((downloadTask) => downloadTask.status === "error"),
);

/** 失败任务按创建时间倒序排列 */
const sortedFailedTasks = computed<DownloadTask[]>(() =>
  [...failedTasks.value].sort(
    (firstTask, secondTask) => secondTask.createdAt - firstTask.createdAt,
  ),
);

interface DateGroup {
  label: string;
  tasks: DownloadTask[];
}

/** 将时间戳格式化为相对日期标签（今天/昨天/前天/年月日） */
const formatDateLabel = (timestamp: number): string => {
  const targetDate = new Date(timestamp);
  const nowDate = new Date();
  const todayZero = new Date(nowDate.getFullYear(), nowDate.getMonth(), nowDate.getDate());
  const targetZero = new Date(
    targetDate.getFullYear(),
    targetDate.getMonth(),
    targetDate.getDate(),
  );
  const diffMilliseconds = todayZero.getTime() - targetZero.getTime();
  const oneDayMilliseconds = 86400000;

  if (diffMilliseconds === 0) return t("downloads.today");
  if (diffMilliseconds === oneDayMilliseconds) return t("downloads.yesterday");
  if (diffMilliseconds === oneDayMilliseconds * 2) return t("downloads.dayBeforeYesterday");
  return `${targetDate.getFullYear()}-${String(targetDate.getMonth() + 1).padStart(2, "0")}-${String(targetDate.getDate()).padStart(2, "0")}`;
};

/** 仅用于已完成历史记录的日期分组 */
const groupByDate = (taskList: DownloadTask[]): DateGroup[] => {
  const sortedList = [...taskList].sort(
    (firstTask, secondTask) => secondTask.createdAt - firstTask.createdAt,
  );
  const groupMap = new Map<string, DownloadTask[]>();
  for (const downloadTask of sortedList) {
    const label = formatDateLabel(downloadTask.createdAt);
    if (!groupMap.has(label)) groupMap.set(label, []);
    groupMap.get(label)!.push(downloadTask);
  }
  return Array.from(groupMap.entries()).map(([label, tasks]) => ({ label, tasks }));
};

const completedGroups = computed<DateGroup[]>(() => groupByDate(completedTasks.value));

/** 清空所有已取消任务记录 */
const handleClearCancelled = () => {
  window.$dialog.warning({
    title: t("downloads.clearCancelled"),
    content: t("downloads.confirmClearCancelled"),
    positiveText: t("common.clear"),
    negativeText: t("common.cancel"),
    onPositiveClick: () => {
      downloadStore.clearCancelled();
    },
  });
};

/** 清空所有下载失败任务记录 */
const handleClearFailed = () => {
  window.$dialog.warning({
    title: t("downloads.clearFailed"),
    content: t("downloads.confirmClearFailed"),
    positiveText: t("common.clear"),
    negativeText: t("common.cancel"),
    onPositiveClick: () => {
      downloadStore.clearFailed();
    },
  });
};

/** 重新尝试所有失败的任务 */
const handleRetryAllFailed = () => {
  window.$dialog.info({
    title: t("downloads.retryAll"),
    content: t("downloads.confirmRetryAllFailed"),
    positiveText: t("downloads.retryAll"),
    negativeText: t("common.cancel"),
    onPositiveClick: async () => {
      try {
        for (const failedTask of failedTasks.value) {
          await downloadStore.retryTask(failedTask.id);
        }
      } catch (error: unknown) {
        window.$message.error(
          error instanceof Error ? error.message : String(error) || t("downloads.retryFailed"),
        );
      }
    },
  });
};
</script>

<template>
  <div class="downloads-page">
    <n-tabs
      v-model:value="activeCategory"
      type="line"
      justify-content="space-evenly"
      class="downloads-tabs"
    >
      <n-tab-pane name="all">
        <template #tab>
          <span>{{ $t("downloads.all") }}</span>
          <span v-if="totalTasksCount > 0" class="tab-count">({{ totalTasksCount }})</span>
        </template>
        <n-scrollbar class="tab-scrollbar">
          <div class="tab-pane-content">
            <div v-if="totalTasksCount === 0" class="section-empty">
              <n-empty :description="$t('downloads.noFinishedTasks')" size="small" />
            </div>
            <n-flex v-else vertical :size="20">
              <!-- 下载中分区 -->
              <div v-if="activeTasks.length > 0" class="section">
                <n-flex align="center" :size="8" class="section-title-bar">
                  <n-icon size="16"><icon-mdi-download /></n-icon>
                  <n-text strong>{{ $t("downloads.downloading") }}</n-text>
                  <n-tag size="small" round :bordered="false" type="info">
                    {{ activeTasks.length }}
                  </n-tag>
                </n-flex>
                <n-flex vertical :size="10">
                  <DownloadCard v-for="task in sortedActiveTasks" :key="task.id" :task="task" />
                </n-flex>
              </div>

              <!-- 已完成分区 -->
              <div v-if="completedTasks.length > 0" class="section">
                <CompletedTasksSection
                  :groups="completedGroups"
                  :total-count="completedTasks.length"
                />
              </div>

              <!-- 已取消分区 -->
              <div v-if="cancelledTasks.length > 0" class="section">
                <n-flex align="center" :size="8" class="section-title-bar">
                  <n-icon size="16"><icon-mdi-cancel /></n-icon>
                  <n-text strong>{{ $t("downloads.cancelledTab") }}</n-text>
                  <n-tag size="small" round :bordered="false" type="default">
                    {{ cancelledTasks.length }}
                  </n-tag>
                  <n-button
                    size="tiny"
                    strong
                    secondary
                    type="error"
                    style="margin-left: auto"
                    @click="handleClearCancelled"
                  >
                    <template #icon>
                      <n-icon size="14"><icon-mdi-delete-sweep-outline /></n-icon>
                    </template>
                    {{ $t("downloads.clearCancelled") }}
                  </n-button>
                </n-flex>
                <n-flex vertical :size="10">
                  <DownloadCard v-for="task in sortedCancelledTasks" :key="task.id" :task="task" />
                </n-flex>
              </div>

              <!-- 下载失败分区 -->
              <div v-if="failedTasks.length > 0" class="section">
                <n-flex align="center" :size="8" class="section-title-bar">
                  <n-icon size="16"><icon-mdi-alert-circle-outline /></n-icon>
                  <n-text strong>{{ $t("downloads.failedTab") }}</n-text>
                  <n-tag size="small" round :bordered="false" type="error">
                    {{ failedTasks.length }}
                  </n-tag>
                  <n-flex align="center" size="small" style="margin-left: auto">
                    <n-button
                      size="tiny"
                      strong
                      secondary
                      type="primary"
                      @click="handleRetryAllFailed"
                    >
                      <template #icon>
                        <n-icon size="14"><icon-mdi-refresh /></n-icon>
                      </template>
                      {{ $t("downloads.retryAll") }}
                    </n-button>
                    <n-button size="tiny" strong secondary type="error" @click="handleClearFailed">
                      <template #icon>
                        <n-icon size="14"><icon-mdi-delete-sweep-outline /></n-icon>
                      </template>
                      {{ $t("downloads.clearFailed") }}
                    </n-button>
                  </n-flex>
                </n-flex>
                <n-flex vertical :size="10">
                  <DownloadCard v-for="task in sortedFailedTasks" :key="task.id" :task="task" />
                </n-flex>
              </div>
            </n-flex>
          </div>
        </n-scrollbar>
      </n-tab-pane>

      <!-- 2. 下载中 Tab -->
      <n-tab-pane name="downloading">
        <template #tab>
          <span>{{ $t("downloads.downloading") }}</span>
          <span v-if="activeTasks.length > 0" class="tab-count">({{ activeTasks.length }})</span>
        </template>
        <n-scrollbar class="tab-scrollbar">
          <div class="tab-pane-content">
            <div v-if="activeTasks.length === 0" class="section-empty">
              <n-empty :description="$t('downloads.noActiveTasks')" size="small" />
            </div>
            <div v-else>
              <n-flex align="center" :size="8" class="section-title-bar">
                <n-icon size="16"><icon-mdi-download /></n-icon>
                <n-text strong>{{ $t("downloads.downloading") }}</n-text>
                <n-tag size="small" round :bordered="false" type="info">
                  {{ activeTasks.length }}
                </n-tag>
              </n-flex>
              <n-flex vertical :size="10">
                <DownloadCard v-for="task in sortedActiveTasks" :key="task.id" :task="task" />
              </n-flex>
            </div>
          </div>
        </n-scrollbar>
      </n-tab-pane>

      <!-- 3. 已完成 Tab -->
      <n-tab-pane name="completed">
        <template #tab>
          <span>{{ $t("downloads.completed") }}</span>
          <span v-if="completedTasks.length > 0" class="tab-count">
            ({{ completedTasks.length }})
          </span>
        </template>
        <n-scrollbar class="tab-scrollbar">
          <div class="tab-pane-content">
            <CompletedTasksSection :groups="completedGroups" :total-count="completedTasks.length" />
          </div>
        </n-scrollbar>
      </n-tab-pane>

      <!-- 4. 已取消 Tab -->
      <n-tab-pane name="cancelled">
        <template #tab>
          <span>{{ $t("downloads.cancelledTab") }}</span>
          <span v-if="cancelledTasks.length > 0" class="tab-count">
            ({{ cancelledTasks.length }})
          </span>
        </template>
        <n-scrollbar class="tab-scrollbar">
          <div class="tab-pane-content">
            <div v-if="cancelledTasks.length === 0" class="section-empty">
              <n-empty :description="$t('downloads.noFinishedTasks')" size="small" />
            </div>
            <div v-else>
              <n-flex align="center" :size="8" class="section-title-bar">
                <n-icon size="16"><icon-mdi-cancel /></n-icon>
                <n-text strong>{{ $t("downloads.cancelledTab") }}</n-text>
                <n-tag size="small" round :bordered="false" type="default">
                  {{ cancelledTasks.length }}
                </n-tag>
                <n-button
                  size="tiny"
                  strong
                  secondary
                  type="error"
                  style="margin-left: auto"
                  @click="handleClearCancelled"
                >
                  <template #icon>
                    <n-icon size="14"><icon-mdi-delete-sweep-outline /></n-icon>
                  </template>
                  {{ $t("downloads.clearCancelled") }}
                </n-button>
              </n-flex>
              <n-flex vertical :size="10">
                <DownloadCard v-for="task in sortedCancelledTasks" :key="task.id" :task="task" />
              </n-flex>
            </div>
          </div>
        </n-scrollbar>
      </n-tab-pane>

      <!-- 5. 下载失败 Tab -->
      <n-tab-pane name="failed">
        <template #tab>
          <span>{{ $t("downloads.failedTab") }}</span>
          <span v-if="failedTasks.length > 0" class="tab-count">({{ failedTasks.length }})</span>
        </template>
        <n-scrollbar class="tab-scrollbar">
          <div class="tab-pane-content">
            <div v-if="failedTasks.length === 0" class="section-empty">
              <n-empty :description="$t('downloads.noFinishedTasks')" size="small" />
            </div>
            <div v-else>
              <n-flex align="center" :size="8" class="section-title-bar">
                <n-icon size="16"><icon-mdi-alert-circle-outline /></n-icon>
                <n-text strong>{{ $t("downloads.failedTab") }}</n-text>
                <n-tag size="small" round :bordered="false" type="error">
                  {{ failedTasks.length }}
                </n-tag>
                <n-flex align="center" size="small" style="margin-left: auto">
                  <n-button
                    size="tiny"
                    strong
                    secondary
                    type="primary"
                    @click="handleRetryAllFailed"
                  >
                    <template #icon>
                      <n-icon size="14"><icon-mdi-refresh /></n-icon>
                    </template>
                    {{ $t("downloads.retryAll") }}
                  </n-button>
                  <n-button size="tiny" strong secondary type="error" @click="handleClearFailed">
                    <template #icon>
                      <n-icon size="14"><icon-mdi-delete-sweep-outline /></n-icon>
                    </template>
                    {{ $t("downloads.clearFailed") }}
                  </n-button>
                </n-flex>
              </n-flex>
              <n-flex vertical :size="10">
                <DownloadCard v-for="task in sortedFailedTasks" :key="task.id" :task="task" />
              </n-flex>
            </div>
          </div>
        </n-scrollbar>
      </n-tab-pane>
    </n-tabs>
  </div>
</template>

<style scoped lang="scss">
.downloads-page {
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.downloads-tabs {
  height: 100%;
  display: flex;
  flex-direction: column;
  min-height: 0;

  :deep(.n-tabs-nav) {
    flex-shrink: 0;
    padding: 12px 16px 0;
  }

  :deep(.n-tabs-pane-wrapper) {
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }

  :deep(.n-tab-pane) {
    height: 100%;
    padding: 0;
    overflow: hidden;
  }
}

.tab-scrollbar {
  height: 100%;
}

.tab-pane-content {
  padding: 16px 16px 24px 16px;
}

.tab-count {
  margin-left: 4px;
  font-size: 12px;
  opacity: 0.75;
  line-height: normal;
}

.section-title-bar {
  margin-bottom: 12px;
}

.section-empty {
  padding: 32px 0;
}
</style>
