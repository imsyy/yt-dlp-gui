<script setup lang="ts">
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useI18n } from "vue-i18n";
import { useChannelArchive } from "@/composables/useChannelArchive";
import { showErrorDialog } from "@/utils/format";
import { goToolList } from "@/utils/toolbox";
import type { ChannelRecord, ChannelVideoRecord } from "@/types";

const { t } = useI18n();
const router = useRouter();

const goBack = () => goToolList(router);

const {
  channels,
  channelsLoading,
  activeChannel,
  activeChannelId,
  videos,
  videosLoading,
  total,
  contentType,
  searchQuery,
  sortBy,
  sortOrder,
  syncTabs,
  sleepInterval,
  syncProgress,
  selectChannel,
  handleChannelAdded,
  startSync,
  cancelSync,
  removeChannel,
  exportVideos,
} = useChannelArchive();

const showAddModal = ref(false);

/** 当前频道的实时同步进度 */
const activeProgress = computed(() =>
  activeChannel.value ? syncProgress.value[activeChannel.value.id] : null,
);

/** 复制视频链接到剪贴板 */
const copyLink = async (video: ChannelVideoRecord) => {
  try {
    await writeText(video.url);
    window.$message.success(t("channelArchive.linkCopied"));
  } catch {
    window.$message.error(t("clipboard.writeFailed"));
  }
};

/** 在系统浏览器中打开链接 */
const openInBrowser = async (url: string) => {
  try {
    await openUrl(url);
  } catch (error: unknown) {
    showErrorDialog(String(error));
  }
};

/** 发送单个视频到主页解析下载 */
const sendToDownload = (video: ChannelVideoRecord) => {
  void router.push({ name: "home", query: { url: video.url, _t: String(Date.now()) } });
};

/** 批量发送选中视频到主页下载 */
const sendBatchToDownload = (selected: ChannelVideoRecord[]) => {
  void router.push({
    name: "home",
    query: {
      urls: JSON.stringify(selected.map((video) => video.url)),
      mode: "batch",
      _t: String(Date.now()),
    },
  });
};

/** 删除频道前二次确认 */
const confirmRemoveChannel = (channel: ChannelRecord) => {
  window.$dialog.warning({
    title: t("channelArchive.deleteChannel"),
    content: t("channelArchive.deleteConfirm", {
      name: channel.title,
      count: channel.videoCount,
    }),
    positiveText: t("common.confirm"),
    negativeText: t("common.cancel"),
    onPositiveClick: () => removeChannel(channel.id),
  });
};
</script>

<template>
  <n-flex vertical :size="12" class="channel-archive-page">
    <n-flex align="center" justify="space-between" :wrap="false">
      <n-flex align="center" :size="8">
        <n-button strong secondary size="small" @click="goBack">
          <template #icon>
            <n-icon><icon-mdi-arrow-left /></n-icon>
          </template>
          {{ $t("common.back") }}
        </n-button>
        <n-text strong style="font-size: 15px">{{ $t("channelArchive.title") }}</n-text>
      </n-flex>

      <n-button type="primary" size="small" @click="showAddModal = true">
        <template #icon>
          <n-icon><icon-mdi-plus /></n-icon>
        </template>
        {{ $t("channelArchive.addChannel") }}
      </n-button>
    </n-flex>

    <n-flex :size="12" :wrap="false" class="archive-body">
      <ChannelListCard
        :channels="channels"
        :loading="channelsLoading"
        :active-id="activeChannelId"
        @select="selectChannel"
        @sync="startSync('full', $event.id)"
        @open="openInBrowser($event.url)"
        @remove="confirmRemoveChannel"
      />

      <n-flex
        v-if="activeChannel"
        vertical
        :size="12"
        style="flex: 1; min-width: 0; min-height: 0"
      >
        <ChannelDetailCard
          v-model:sync-tabs="syncTabs"
          v-model:sleep-interval="sleepInterval"
          :channel="activeChannel"
          :progress="activeProgress"
          @sync="startSync"
          @cancel="cancelSync(activeChannel.id)"
          @export="exportVideos"
          @open="openInBrowser(activeChannel.url)"
        />

        <ChannelVideoTable
          v-model:content-type="contentType"
          v-model:search-query="searchQuery"
          v-model:sort-by="sortBy"
          v-model:sort-order="sortOrder"
          :videos="videos"
          :loading="videosLoading"
          :total="total"
          @send="sendToDownload"
          @batch-send="sendBatchToDownload"
          @copy="copyLink"
          @open="openInBrowser($event.url)"
        />
      </n-flex>

      <n-flex v-else vertical justify="center" align="center" style="flex: 1">
        <n-empty :description="$t('channelArchive.selectChannelTip')" />
      </n-flex>
    </n-flex>

    <ChannelAddModal v-model:show="showAddModal" @success="handleChannelAdded" />
  </n-flex>
</template>

<style scoped lang="scss">
/* 撑满可用高度，左右两栏各自内部滚动，页面本身不滚动 */
.channel-archive-page {
  height: 100%;
  min-height: 0;
}

.archive-body {
  flex: 1;
  min-height: 0;
}
</style>
