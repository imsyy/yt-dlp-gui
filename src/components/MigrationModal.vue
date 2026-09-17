<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { useDownloadStore } from "@/stores/download";
import { useHistoryStore } from "@/stores/history";
import { detectLegacyData, migrateLegacyData } from "@/migration";
import type { LegacyDataSummary, MigrationProgress } from "@/migration";

type ModalPhase = "prompt" | "migrating" | "error";

const { t } = useI18n();
const downloadStore = useDownloadStore();
const historyStore = useHistoryStore();

const show = ref(false);
const phase = ref<ModalPhase>("prompt");
const summary = ref<LegacyDataSummary>({ tasks: 0, history: 0 });
const progressText = ref("");
const errorMessage = ref("");

/** 启动时检测老数据，需要用户决策才弹窗，检测失败静默跳过 */
onMounted(async () => {
  try {
    const result = await detectLegacyData();
    if (result.needsPrompt) {
      summary.value = result.summary;
      phase.value = "prompt";
      show.value = true;
    }
  } catch {
    // 检测失败不打扰用户
  }
});

/** 稍后：本次不再询问，下次启动重新检测 */
const handleLater = (): void => {
  show.value = false;
};

const describeProgress = (progress: MigrationProgress): void => {
  progressText.value =
    progress.domain === "tasks" ? t("migration.migratingTasks") : t("migration.migratingHistory");
};

/** 确认迁移：搬运 → 校验 → 重载 store 列表 */
const handleMigrate = async (): Promise<void> => {
  phase.value = "migrating";
  errorMessage.value = "";
  progressText.value = t("migration.migratingTasks");
  try {
    await migrateLegacyData(summary.value, describeProgress);
    await downloadStore.loadTasks();
    await historyStore.loadHistory();
    window.$message.success(t("migration.success"));
    show.value = false;
  } catch (error: unknown) {
    errorMessage.value = error instanceof Error ? error.message : String(error);
    phase.value = "error";
  }
};
</script>

<template>
  <n-modal
    v-model:show="show"
    preset="card"
    :title="$t('migration.title')"
    size="small"
    :bordered="false"
    :mask-closable="false"
    :closable="false"
    content-scrollable
    :segmented="{ action: 'soft' }"
    :style="{ width: '460px', maxHeight: '80vh' }"
  >
    <!-- 确认态 -->
    <n-flex v-if="phase === 'prompt'" vertical :size="16">
      <n-alert type="info" :bordered="false">
        {{ $t("migration.description") }}
      </n-alert>
      <n-flex vertical :size="8">
        <n-text v-if="summary.tasks > 0">
          {{ $t("migration.tasksCount", { n: summary.tasks }) }}
        </n-text>
        <n-text v-if="summary.history > 0">
          {{ $t("migration.historyCount", { n: summary.history }) }}
        </n-text>
      </n-flex>
    </n-flex>

    <!-- 迁移中 -->
    <n-flex v-else-if="phase === 'migrating'" vertical align="center" :size="16">
      <n-spin size="medium" />
      <n-text depth="3">{{ progressText }}</n-text>
    </n-flex>

    <!-- 失败态 -->
    <n-flex v-else vertical :size="16">
      <n-alert type="error" :bordered="false">
        {{ $t("migration.failed", { message: errorMessage }) }}
      </n-alert>
    </n-flex>

    <template #action>
      <n-flex justify="end">
        <template v-if="phase === 'migrating'">
          <n-button disabled>{{ $t("migration.migrateNow") }}</n-button>
        </template>
        <template v-else>
          <n-button strong secondary @click="handleLater">{{ $t("setup.later") }}</n-button>
          <n-button type="primary" @click="handleMigrate">
            {{ phase === "error" ? $t("migration.retry") : $t("migration.migrateNow") }}
          </n-button>
        </template>
      </n-flex>
    </template>
  </n-modal>
</template>
