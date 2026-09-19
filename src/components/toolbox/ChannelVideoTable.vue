<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { NButton, NEllipsis, NFlex, NIcon, NText, NTooltip } from "naive-ui";
import IconMdiContentCopy from "~icons/mdi/content-copy";
import IconMdiDownload from "~icons/mdi/download";
import IconMdiOpenInNew from "~icons/mdi/open-in-new";
import IconMdiSortAscending from "~icons/mdi/sort-ascending";
import IconMdiSortDescending from "~icons/mdi/sort-descending";
import { formatDateYMD, formatDuration, formatViewCount } from "@/utils/format";
import type {
  ChannelSortBy,
  ChannelSortOrder,
  ChannelVideoType,
} from "@/composables/useChannelArchive";
import type { ChannelVideoRecord } from "@/types";
import type { Component } from "vue";
import type { DataTableColumns, DataTableRowKey, SelectOption } from "naive-ui";

const { t } = useI18n();

const props = defineProps<{
  videos: ChannelVideoRecord[];
  loading: boolean;
  /** 当前筛选条件下的归档总数 */
  total: number;
}>();

const emit = defineEmits<{
  (e: "send", video: ChannelVideoRecord): void;
  (e: "batch-send", videos: ChannelVideoRecord[]): void;
  (e: "copy", video: ChannelVideoRecord): void;
  (e: "open", video: ChannelVideoRecord): void;
}>();

const contentType = defineModel<ChannelVideoType>("contentType", { required: true });
const searchQuery = defineModel<string>("searchQuery", { required: true });
const sortBy = defineModel<ChannelSortBy>("sortBy", { required: true });
const sortOrder = defineModel<ChannelSortOrder>("sortOrder", { required: true });

const checkedKeys = ref<DataTableRowKey[]>([]);

const checkedVideos = computed(() => {
  const keys = new Set(checkedKeys.value);
  return props.videos.filter((video) => keys.has(video.id));
});

// 列表重新加载后旧选中项已不存在，直接清空
watch(
  () => props.videos,
  () => {
    checkedKeys.value = [];
  },
);

const rowKey = (video: ChannelVideoRecord) => video.id;

const sortOptions = computed<SelectOption[]>(() => [
  { label: t("channelArchive.sortPublished"), value: "published_at" },
  { label: t("channelArchive.sortViews"), value: "view_count" },
  { label: t("channelArchive.sortDuration"), value: "duration" },
]);

const toggleSortOrder = () => {
  sortOrder.value = sortOrder.value === "desc" ? "asc" : "desc";
};

/** 统一渲染浅色横杠占位符 */
const renderPlaceholder = () => h(NText, { depth: 3 }, { default: () => "-" });

/** 行内操作按钮：图标按钮 + 悬浮提示 */
const renderAction = (icon: Component, label: string, onClick: () => void) =>
  h(
    NTooltip,
    { trigger: "hover" },
    {
      trigger: () =>
        h(
          NButton,
          { quaternary: true, circle: true, size: "small", onClick },
          { icon: () => h(NIcon, null, { default: () => h(icon) }) },
        ),
      default: () => label,
    },
  );

const columns = computed<DataTableColumns<ChannelVideoRecord>>(() => [
  { type: "selection" },
  {
    title: t("channelArchive.videoTitle"),
    key: "title",
    minWidth: 280,
    render: (row) =>
      h(
        NFlex,
        { align: "center", size: 10, wrap: false },
        {
          default: () => [
            row.thumbnail
              ? h("img", {
                  class: "video-thumb",
                  src: row.thumbnail,
                  alt: "",
                  decoding: "async",
                  referrerPolicy: "no-referrer",
                })
              : h("div", { class: "video-thumb empty-thumb" }),
            h(NEllipsis, { lineClamp: 2, tooltip: true }, { default: () => row.title || "-" }),
          ],
        },
      ),
  },
  {
    title: t("channelArchive.sortDuration"),
    key: "duration",
    width: 100,
    // yt-dlp 时长是浮点秒，沿用历史行为先四舍五入再展示
    render: (row) =>
      formatDuration(row.duration == null ? row.duration : Math.round(row.duration)) ||
      renderPlaceholder(),
  },
  {
    title: t("channelArchive.sortViews"),
    key: "viewCount",
    width: 110,
    render: (row) => (row.viewCount ? formatViewCount(row.viewCount) : renderPlaceholder()),
  },
  {
    title: t("channelArchive.sortPublished"),
    key: "publishedAt",
    width: 120,
    render: (row) => {
      const text = formatDateYMD(row.publishedAt);
      if (!text) return renderPlaceholder();
      // 近似日期加 ~ 前缀，校准后恢复精确显示
      return row.publishedAccuracy === "exact" ? text : `~${text}`;
    },
  },
  {
    title: t("channelArchive.actions"),
    key: "actions",
    align: "center",
    width: 130,
    render: (row) =>
      h(
        NFlex,
        { align: "center", size: 12, wrap: false },
        {
          default: () => [
            renderAction(IconMdiDownload, t("channelArchive.sendToDownload"), () =>
              emit("send", row),
            ),
            renderAction(IconMdiContentCopy, t("channelArchive.copyLink"), () => emit("copy", row)),
            renderAction(IconMdiOpenInNew, t("channelArchive.openInBrowser"), () =>
              emit("open", row),
            ),
          ],
        },
      ),
  },
]);
</script>

<template>
  <n-card
    size="small"
    :title="$t('channelArchive.videoCount', { count: total })"
    class="video-table-card"
  >
    <template #header-extra>
      <n-radio-group v-model:value="contentType" size="small">
        <n-radio-button value="video">{{ $t("channelArchive.typeVideo") }}</n-radio-button>
        <n-radio-button value="stream">{{ $t("channelArchive.typeStream") }}</n-radio-button>
        <n-radio-button value="short">{{ $t("channelArchive.typeShort") }}</n-radio-button>
      </n-radio-group>
    </template>

    <n-flex align="center" justify="space-between" :size="8" :wrap="false">
      <n-flex align="center" :size="8" :wrap="false">
        <n-input
          v-model:value="searchQuery"
          clearable
          size="small"
          style="width: 220px"
          :placeholder="$t('channelArchive.searchPlaceholder')"
        >
          <template #prefix>
            <n-icon><icon-mdi-magnify /></n-icon>
          </template>
        </n-input>

        <n-select v-model:value="sortBy" size="small" style="width: 130px" :options="sortOptions" />

        <n-button secondary size="small" @click="toggleSortOrder">
          <template #icon>
            <n-icon>
              <component
                :is="sortOrder === 'desc' ? IconMdiSortDescending : IconMdiSortAscending"
              />
            </n-icon>
          </template>
          {{ sortOrder === "desc" ? $t("channelArchive.sortDesc") : $t("channelArchive.sortAsc") }}
        </n-button>
      </n-flex>

      <n-button
        type="primary"
        secondary
        size="small"
        :disabled="checkedVideos.length === 0"
        @click="emit('batch-send', checkedVideos)"
      >
        <template #icon>
          <n-icon><icon-mdi-download /></n-icon>
        </template>
        {{ $t("channelArchive.sendToDownload") }}{{ checkedVideos.length > 0 ? ` (${checkedVideos.length})` : "" }}
      </n-button>
    </n-flex>

    <n-data-table
      v-model:checked-row-keys="checkedKeys"
      :columns="columns"
      :data="videos"
      :row-key="rowKey"
      :loading="loading"
      flex-height
      virtual-scroll
      size="small"
      bordered
    >
      <template #empty>
        <n-empty size="small" :description="$t('channelArchive.emptyVideos')" />
      </template>
    </n-data-table>
  </n-card>
</template>

<style scoped lang="scss">
.video-table-card {
  flex: 1;
  min-height: 0;

  :deep(.n-card-content) {
    display: flex;
    flex-direction: column;
    gap: 10px;
    overflow: hidden;
  }

  :deep(.n-data-table) {
    flex: 1;
    min-height: 0;
  }
}

.video-table-card :deep(.video-thumb) {
  flex-shrink: 0;
  width: 84px;
  height: 48px;
  border-radius: 4px;
  object-fit: cover;
  background-color: var(--n-merged-td-color-hover);
}
</style>
