<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { formatViewCount } from "@/utils/format";
import type { ChannelSyncMode, ChannelExportFormat } from "@/composables/useChannelArchive";
import type { ChannelRecord, ChannelSyncProgressPayload } from "@/types";
import type { DropdownOption } from "naive-ui";

const { t } = useI18n();

const props = defineProps<{
  channel: ChannelRecord;
  progress?: ChannelSyncProgressPayload | null;
}>();

const emit = defineEmits<{
  (e: "sync", mode: ChannelSyncMode): void;
  (e: "cancel"): void;
  (e: "export", format: ChannelExportFormat): void;
  (e: "open"): void;
}>();

/** 同步分区勾选与请求间隔由页面持有，供启动同步时读取 */
const syncTabs = defineModel<string[]>("syncTabs", { required: true });
const sleepInterval = defineModel<number>("sleepInterval", { required: true });

const syncing = computed(() => props.channel.syncStatus === "syncing");

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
</script>

<template>
  <n-card size="small" class="channel-detail-card">
    <n-flex vertical :size="12">
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
              <n-button quaternary circle size="tiny" @click="emit('open')">
                <template #icon>
                  <n-icon><icon-mdi-open-in-new /></n-icon>
                </template>
              </n-button>
            </n-flex>

            <n-flex align="center" :size="10" :wrap="false" class="detail-meta">
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
          <n-button
            v-if="syncing"
            type="warning"
            secondary
            size="small"
            @click="emit('cancel')"
          >
            <template #icon>
              <n-icon><icon-mdi-cancel /></n-icon>
            </template>
            {{ $t("channelArchive.cancelSync") }}
          </n-button>

          <template v-else>
            <n-button type="primary" size="small" @click="emit('sync', 'incremental')">
              <template #icon>
                <n-icon><icon-mdi-sync /></n-icon>
              </template>
              {{ $t("channelArchive.incrementalSync") }}
            </n-button>
            <n-button secondary size="small" @click="emit('sync', 'full')">
              {{ $t("channelArchive.fullSync") }}
            </n-button>
          </template>

          <n-popover trigger="click" placement="bottom-end">
            <template #trigger>
              <n-button quaternary circle size="small" :disabled="syncing">
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
            :options="exportOptions"
            @select="(key: ChannelExportFormat) => emit('export', key)"
          >
            <n-button secondary size="small">
              <template #icon>
                <n-icon><icon-mdi-export /></n-icon>
              </template>
              {{ $t("channelArchive.exportVideos") }}
            </n-button>
          </n-dropdown>
        </n-flex>
      </n-flex>

      <n-alert v-if="syncing" type="warning" :bordered="false">
        {{
          $t("channelArchive.syncProgress", {
            total: progress?.totalSynced ?? 0,
            added: progress?.newSynced ?? 0,
          })
        }}
      </n-alert>
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
