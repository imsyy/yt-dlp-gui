<script setup lang="ts">
import { useDownloadStore } from "@/stores/download";
import { useI18n } from "vue-i18n";
import type { DownloadTask } from "@/types";

interface DateGroup {
  label: string;
  tasks: DownloadTask[];
}

interface Props {
  groups: DateGroup[];
  totalCount: number;
  showHeader?: boolean;
}

withDefaults(defineProps<Props>(), {
  showHeader: true,
});

const { t } = useI18n();
const downloadStore = useDownloadStore();

const handleClearCompleted = () => {
  window.$dialog.warning({
    title: t("downloads.clearCompleted"),
    content: t("downloads.confirmClearCompleted"),
    positiveText: t("common.clear"),
    negativeText: t("common.cancel"),
    onPositiveClick: () => {
      downloadStore.clearCompleted();
    },
  });
};
</script>

<template>
  <div class="completed-section">
    <!-- 标题操作栏 -->
    <n-flex v-if="showHeader" align="center" :size="8" class="section-title-bar">
      <n-icon size="16"><icon-mdi-check-circle-outline /></n-icon>
      <n-text strong>{{ $t("downloads.completed") }}</n-text>
      <n-tag v-if="totalCount > 0" size="small" round :bordered="false" type="success">
        {{ totalCount }}
      </n-tag>
      <n-button
        v-if="totalCount > 0"
        size="tiny"
        strong
        secondary
        type="error"
        style="margin-left: auto"
        @click="handleClearCompleted"
      >
        <template #icon>
          <n-icon size="14"><icon-mdi-delete-sweep-outline /></n-icon>
        </template>
        {{ $t("downloads.clearCompleted") }}
      </n-button>
    </n-flex>

    <!-- 空状态 -->
    <div v-if="totalCount === 0" class="section-empty">
      <n-empty :description="$t('downloads.noFinishedTasks')" size="small" />
    </div>

    <!-- 按日期分组列表 -->
    <template v-else>
      <div v-for="group in groups" :key="'completed-' + group.label" class="date-group">
        <n-text depth="3" class="date-label">{{ group.label }}</n-text>
        <n-flex vertical :size="10">
          <DownloadCard v-for="task in group.tasks" :key="task.id" :task="task" />
        </n-flex>
      </div>
    </template>
  </div>
</template>

<style scoped lang="scss">
.section-title-bar {
  margin-bottom: 12px;
}

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
</style>
