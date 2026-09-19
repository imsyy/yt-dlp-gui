<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { NIcon } from "naive-ui";
import IconMdiDeleteOutline from "~icons/mdi/delete-outline";
import IconMdiOpenInNew from "~icons/mdi/open-in-new";
import type { ChannelRecord } from "@/types";
import type { DropdownOption } from "naive-ui";
import type { Component } from "vue";

const { t } = useI18n();

const props = defineProps<{
  channels: ChannelRecord[];
  loading: boolean;
  activeId: string | null;
  /** 正在日期校准的频道 ID（校准不写 syncStatus，单独透传） */
  enrichingIds?: string[];
}>();

/** 指定频道是否正在日期校准：校准期间同样禁用同步菜单，与主按钮行为对齐 */
const isEnriching = (channelId: string) => (props.enrichingIds ?? []).includes(channelId);

const emit = defineEmits<{
  (e: "select", channelId: string): void;
  (e: "open", channel: ChannelRecord): void;
  (e: "remove", channel: ChannelRecord): void;
}>();

/** 平台标识的展示名 */
const platformLabel: Record<string, string> = {
  youtube: "YouTube",
  bilibili: "Bilibili",
};

const platformOf = (platform: string) => platformLabel[platform] ?? platform;

/** 下拉菜单图标渲染 */
const menuIcon = (icon: Component) => () => h(NIcon, null, { default: () => h(icon) });

/** 频道操作菜单：同步统一走右侧详情区，这里只保留打开与删除 */
const menuOptions = computed<DropdownOption[]>(() => [
  {
    label: t("channelArchive.openInBrowser"),
    key: "open",
    icon: menuIcon(IconMdiOpenInNew),
  },
  { type: "divider", key: "divider" },
  {
    label: t("channelArchive.deleteChannel"),
    key: "remove",
    icon: menuIcon(IconMdiDeleteOutline),
  },
]);

const handleMenuSelect = (key: string, channel: ChannelRecord) => {
  if (key === "open") emit("open", channel);
  else if (key === "remove") emit("remove", channel);
};
</script>

<template>
  <n-card size="small" :title="$t('channelArchive.channelList')" class="channel-list-card">
    <template #header-extra>
      <n-tag size="tiny" :bordered="false" round>{{ channels.length }}</n-tag>
    </template>

    <n-scrollbar>
      <n-spin :show="loading">
        <n-empty
          v-if="channels.length === 0"
          size="small"
          :description="$t('channelArchive.emptyChannels')"
        />

        <n-flex v-else vertical :size="6" class="channel-list">
          <n-flex
            v-for="channel in channels"
            :key="channel.id"
            align="center"
            :size="8"
            :wrap="false"
            class="channel-item"
            :class="{ 'is-active': channel.id === activeId }"
            @click="emit('select', channel.id)"
          >
            <n-badge
              :show="channel.syncStatus === 'syncing' || isEnriching(channel.id)"
              dot
              type="warning"
              :offset="[-4, 4]"
              processing
            >
              <n-avatar round :size="30" :src="channel.avatar || undefined">
                <template #fallback>
                  <n-icon :size="16"><icon-mdi-account-circle /></n-icon>
                </template>
              </n-avatar>
            </n-badge>

            <n-flex vertical :size="0" class="channel-info" align="stretch">
              <n-ellipsis :tooltip="true" class="channel-name">
                {{ channel.title }}
              </n-ellipsis>
              <n-ellipsis :tooltip="false" class="channel-meta">
                {{ platformOf(channel.platform) }} ·
                {{ $t("channelArchive.videoCount", { count: channel.videoCount }) }}
              </n-ellipsis>
            </n-flex>

            <n-dropdown
              trigger="click"
              :options="menuOptions"
              @select="(key: string) => handleMenuSelect(key, channel)"
            >
              <n-button quaternary circle size="tiny" @click.stop>
                <template #icon>
                  <n-icon><icon-mdi-dots-vertical /></n-icon>
                </template>
              </n-button>
            </n-dropdown>
          </n-flex>
        </n-flex>
      </n-spin>
    </n-scrollbar>
  </n-card>
</template>

<style scoped lang="scss">
.channel-list-card {
  width: 260px;
  flex-shrink: 0;

  :deep(.n-card-content) {
    overflow: hidden;
    display: flex;
    flex-direction: column;
    padding: 0 8px 8px;
  }

  :deep(.n-scrollbar) {
    flex: 1;
    min-height: 0;
  }
}

.channel-item {
  flex: 0 0 auto;
  padding: 6px 8px;
  cursor: pointer;
  overflow: hidden;
  border-radius: 6px;
  transition: background-color 0.15s ease;

  &:hover,
  &.is-active {
    background-color: var(--n-action-color);
  }
}

.channel-info {
  flex: 1;
  min-width: 0;
}

.channel-name {
  font-size: 13px;
  line-height: normal;
}

.channel-meta {
  font-size: 12px;
}
</style>
