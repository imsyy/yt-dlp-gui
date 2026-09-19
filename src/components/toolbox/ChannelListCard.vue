<script setup lang="ts">
import { useI18n } from "vue-i18n";
import type { ChannelRecord } from "@/types";
import type { DropdownOption } from "naive-ui";

const { t } = useI18n();

defineProps<{
  channels: ChannelRecord[];
  loading: boolean;
  activeId: string | null;
}>();

const emit = defineEmits<{
  (e: "select", channelId: string): void;
  (e: "sync", channel: ChannelRecord): void;
  (e: "open", channel: ChannelRecord): void;
  (e: "remove", channel: ChannelRecord): void;
}>();

/** 平台标识的展示名与标签配色 */
const platformMeta: Record<string, { label: string; type: "error" | "info" | "default" }> = {
  youtube: { label: "YouTube", type: "error" },
  bilibili: { label: "Bilibili", type: "info" },
};

const platformOf = (platform: string) =>
  platformMeta[platform] ?? { label: platform, type: "default" as const };

/** 频道操作菜单 */
const menuOptions = (channel: ChannelRecord): DropdownOption[] => [
  {
    label: t("channelArchive.fullSync"),
    key: "sync",
    disabled: channel.syncStatus === "syncing",
  },
  { label: t("channelArchive.openInBrowser"), key: "open" },
  { type: "divider", key: "divider" },
  { label: t("channelArchive.deleteChannel"), key: "remove" },
];

const handleMenuSelect = (key: string, channel: ChannelRecord) => {
  if (key === "sync") emit("sync", channel);
  else if (key === "open") emit("open", channel);
  else if (key === "remove") emit("remove", channel);
};
</script>

<template>
  <n-card size="small" :title="$t('channelArchive.channelList')" class="channel-list-card">
    <template #header-extra>
      <n-text depth="3" class="channel-count">{{ channels.length }}</n-text>
    </template>

    <n-spin :show="loading">
      <n-empty
        v-if="channels.length === 0"
        size="small"
        :description="$t('channelArchive.emptyChannels')"
      />

      <n-list v-else hoverable class="channel-list">
        <n-list-item
          v-for="channel in channels"
          :key="channel.id"
          class="channel-item"
          :class="{ 'is-active': channel.id === activeId }"
          @click="emit('select', channel.id)"
        >
          <n-flex align="center" :size="10" :wrap="false">
            <n-avatar round :size="36" :src="channel.avatar || undefined">
              <template #fallback>
                <n-icon :size="20"><icon-mdi-account-circle /></n-icon>
              </template>
            </n-avatar>

            <n-flex vertical :size="2" class="channel-info">
              <n-ellipsis :tooltip="true" class="channel-name">
                {{ channel.title }}
              </n-ellipsis>
              <n-flex align="center" :size="6" :wrap="false">
                <n-tag
                  size="tiny"
                  round
                  :bordered="false"
                  :type="platformOf(channel.platform).type"
                >
                  {{ platformOf(channel.platform).label }}
                </n-tag>
                <n-text depth="3" class="channel-meta">
                  {{ $t("channelArchive.videoCount", { count: channel.videoCount }) }}
                </n-text>
              </n-flex>
            </n-flex>

            <n-tag
              v-if="channel.syncStatus === 'syncing'"
              size="tiny"
              round
              :bordered="false"
              type="warning"
            >
              {{ $t("channelArchive.syncing") }}
            </n-tag>

            <n-dropdown
              trigger="click"
              :options="menuOptions(channel)"
              @select="(key: string) => handleMenuSelect(key, channel)"
            >
              <n-button quaternary circle size="tiny" @click.stop>
                <template #icon>
                  <n-icon><icon-mdi-dots-vertical /></n-icon>
                </template>
              </n-button>
            </n-dropdown>
          </n-flex>
        </n-list-item>
      </n-list>
    </n-spin>
  </n-card>
</template>

<style scoped lang="scss">
.channel-list-card {
  width: 280px;
  flex-shrink: 0;

  /* overflow 非 visible 时 flex 项的自动最小高度为 0，卡片主体因此可内部滚动 */
  :deep(.n-card-content) {
    overflow: auto;
  }
}

.channel-count {
  font-size: 12px;
}

.channel-info {
  flex: 1;
  min-width: 0;
}

.channel-name {
  font-size: 13px;
  font-weight: 500;
}

.channel-meta {
  font-size: 11px;
}

/* 选中项复用列表自身的悬浮底色，随主题变化 */
.channel-list :deep(.channel-item.is-active) {
  background-color: var(--n-merged-color-hover);
}
</style>
