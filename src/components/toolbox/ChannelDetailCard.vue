<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { NIcon } from "naive-ui";
import IconMdiCalendarClock from "~icons/mdi/calendar-clock";
import IconMdiCloudSyncOutline from "~icons/mdi/cloud-sync-outline";
import IconMdiSync from "~icons/mdi/sync";
import { formatViewCount } from "@/utils/format";
import type { ChannelSyncMode, ChannelExportFormat } from "@/composables/useChannelArchive";
import type { ChannelRecord, ChannelSyncProgressPayload } from "@/types";
import type { DropdownOption } from "naive-ui";
import type { Component } from "vue";

const { t } = useI18n();

const props = defineProps<{
  channel: ChannelRecord;
  progress?: ChannelSyncProgressPayload | null;
  /** 是否正在日期校准：与同步共用同一个“忙”状态，进度文本按类型显示 */
  enriching?: boolean;
  enrichText?: string | null;
}>();

const emit = defineEmits<{
  (e: "sync", mode: ChannelSyncMode): void;
  (e: "cancel"): void;
  (e: "cancelEnrich"): void;
  (e: "enrichGap"): void;
  (e: "export", format: ChannelExportFormat): void;
  (e: "open"): void;
}>();

/** 同步分区勾选与请求间隔由页面持有，供启动同步时读取 */
const syncTabs = defineModel<string[]>("syncTabs", { required: true });
const sleepInterval = defineModel<number>("sleepInterval", { required: true });

const syncing = computed(() => props.channel.syncStatus === "syncing");

/** 统一忙态：同步中或校准中都只表达为“加载中”，不区分任务种类 */
const busy = computed(() => syncing.value || props.enriching === true);

/** 忙时进度明细：任务槽位单一，但按类型显示对应进度 */
const busyDetail = computed(() => {
  if (syncing.value) return syncStatusText.value;
  return props.enrichText ?? t("channelArchive.loading");
});

const lastSyncedText = computed(() =>
  props.channel.lastSyncedAt
    ? t("channelArchive.lastSynced", {
        time: new Date(props.channel.lastSyncedAt).toLocaleString(),
      })
    : t("channelArchive.neverSynced"),
);

const exportOptions = computed<DropdownOption[]>(() => [
  { label: t("channelArchive.exportJson"), key: "json" },
  { label: t("channelArchive.exportCsv"), key: "csv" },
]);

/** 同步进度明细 */
const syncStatusText = computed(() => {
  const p = props.progress;
  if (!p) return t("channelArchive.loading");
  const tabMap: Record<string, string> = {
    videos: t("channelArchive.typeVideo"),
    shorts: t("channelArchive.typeShort"),
    streams: t("channelArchive.typeStream"),
  };
  const tabLabel = p.currentTab ? (tabMap[p.currentTab] ?? p.currentTab) : "";
  const detail = t("channelArchive.syncProgress", {
    total: p.totalSynced,
    added: p.newSynced,
  });
  return tabLabel ? `${tabLabel} · ${detail}` : detail;
});

/** 全量同步二次确认 */
const handleFullSyncClick = () => {
  window.$dialog.warning({
    title: t("channelArchive.fullSync"),
    content: t("channelArchive.fullSyncConfirm"),
    positiveText: t("common.confirm"),
    negativeText: t("common.cancel"),
    onPositiveClick: () => emit("sync", "full"),
  });
};

/** 查漏补缺（重校模糊日期）二次确认：目标数由后端返回，无需前端计数 */
const handleEnrichGapClick = () => {
  window.$dialog.warning({
    title: t("channelArchive.gapFill"),
    content: t("channelArchive.gapFillConfirm"),
    positiveText: t("common.confirm"),
    negativeText: t("common.cancel"),
    onPositiveClick: () => emit("enrichGap"),
  });
};

/** 下拉菜单图标渲染 */
const menuIcon = (icon: Component) => () => h(NIcon, null, { default: () => h(icon) });

/** 同步方式下拉菜单：所有功能都在菜单里，主按钮只负责打开菜单与统一加载态 */
const syncMenuOptions = computed<DropdownOption[]>(() => [
  {
    label: t("channelArchive.incrementalSync"),
    key: "incremental",
    icon: menuIcon(IconMdiSync),
  },
  { label: t("channelArchive.fullSync"), key: "full", icon: menuIcon(IconMdiCloudSyncOutline) },
  {
    label: t("channelArchive.gapFill"),
    key: "enrich",
    icon: menuIcon(IconMdiCalendarClock),
    // 自动重查兜底跑着的时候手动项禁用，避免重复提交（后端也会拒绝）
    disabled: props.enriching,
  },
]);

const handleSyncMenuSelect = (key: string) => {
  if (key === "full") handleFullSyncClick();
  else if (key === "enrich") handleEnrichGapClick();
  else emit("sync", "incremental");
};
</script>

<template>
  <n-card size="small" class="channel-detail-card">
    <n-flex align="center" justify="space-between" :size="12" :wrap="false">
      <n-flex align="center" :size="12" :wrap="false" class="detail-main">
        <n-avatar round :size="44" :src="channel.avatar || undefined">
          <template #fallback>
            <n-icon :size="24"><icon-mdi-account-circle /></n-icon>
          </template>
        </n-avatar>

        <n-flex vertical :size="2" class="detail-info">
          <n-flex align="center" :size="2" :wrap="false">
            <n-ellipsis :tooltip="true" class="detail-title">
              {{ channel.title }}
            </n-ellipsis>
            <n-button quaternary circle size="tiny" :disabled="syncing" @click="emit('open')">
              <template #icon>
                <n-icon><icon-mdi-open-in-new /></n-icon>
              </template>
            </n-button>
          </n-flex>

          <n-flex v-if="busy" align="center" :size="6" :wrap="false" class="detail-meta syncing-meta">
            <n-spin :size="13" />
            <n-text depth="2" class="syncing-text">
              {{ busyDetail }}
            </n-text>
          </n-flex>
          <n-flex v-else align="center" :size="10" :wrap="false" class="detail-meta">
            <n-text v-if="channel.uploaderId" depth="3">{{ channel.uploaderId }}</n-text>
            <n-text v-if="channel.subscriberCount" depth="3">
              {{
                $t("channelArchive.subscribers", {
                  count: formatViewCount(channel.subscriberCount),
                })
              }}
            </n-text>
            <n-text depth="3">{{ lastSyncedText }}</n-text>
          </n-flex>
        </n-flex>
      </n-flex>

      <n-flex align="center" :size="8" :wrap="false">
        <!-- 同步入口：按钮只负责打开菜单与统一加载态，具体功能都在下拉菜单里。
             忙时入口禁用，与列表菜单行为一致 -->
        <n-dropdown
          trigger="click"
          :disabled="busy"
          :options="syncMenuOptions"
          @select="(key: string) => handleSyncMenuSelect(key)"
        >
          <n-button type="primary" size="small" :loading="busy" :disabled="busy">
            <template #icon>
              <n-icon><icon-mdi-sync /></n-icon>
            </template>
            {{ busy ? $t("channelArchive.loading") : $t("channelArchive.sync") }}
          </n-button>
        </n-dropdown>
        <!-- 忙时取消：同步中取消同步，纯校准时取消校准 -->
        <n-button
          v-if="busy"
          type="warning"
          secondary
          size="small"
          @click="syncing ? emit('cancel') : emit('cancelEnrich')"
        >
          <template #icon>
            <n-icon><icon-mdi-cancel /></n-icon>
          </template>
          {{ $t("common.cancel") }}
        </n-button>

        <n-popover trigger="click" placement="bottom-end" :disabled="busy">
          <template #trigger>
            <n-button quaternary circle size="small" :disabled="busy">
              <template #icon>
                <n-icon><icon-mdi-cog-outline /></n-icon>
              </template>
            </n-button>
          </template>

          <n-flex vertical :size="12" class="sync-settings">
            <n-text strong>{{ $t("channelArchive.syncOptions") }}</n-text>
            <n-flex vertical :size="6">
              <n-text depth="3" class="sync-settings-label">
                {{ $t("channelArchive.syncTabs") }}
              </n-text>
              <n-checkbox-group v-model:value="syncTabs">
                <n-flex vertical :size="6">
                  <n-checkbox value="videos" :label="$t('channelArchive.typeVideo')" />
                  <n-checkbox value="shorts" :label="$t('channelArchive.typeShort')" />
                  <n-checkbox value="streams" :label="$t('channelArchive.typeStream')" />
                </n-flex>
              </n-checkbox-group>
            </n-flex>
            <n-flex vertical :size="6">
              <n-text depth="3" class="sync-settings-label">
                {{ $t("channelArchive.syncRateLimit") }}
              </n-text>
              <n-slider
                v-model:value="sleepInterval"
                :min="0"
                :max="3"
                :step="0.5"
                :marks="{ 0: '0s', 1: '1s', 2: '2s', 3: '3s' }"
              />
            </n-flex>
          </n-flex>
        </n-popover>

        <n-dropdown
          trigger="click"
          :disabled="busy"
          :options="exportOptions"
          @select="(key: ChannelExportFormat) => emit('export', key)"
        >
          <n-button secondary size="small" :disabled="busy">
            <template #icon>
              <n-icon><icon-mdi-export /></n-icon>
            </template>
            {{ $t("channelArchive.exportVideos") }}
          </n-button>
        </n-dropdown>
      </n-flex>
    </n-flex>
  </n-card>
</template>

<style scoped lang="scss">
.detail-main {
  flex: 1;
  min-width: 0;
}

.detail-info {
  flex: 1;
  min-width: 0;
}

.detail-title {
  font-size: 15px;
  font-weight: 500;
}

.detail-meta {
  font-size: 12px;
}

.sync-settings {
  width: 220px;
}

.sync-settings-label {
  font-size: 12px;
}
</style>
