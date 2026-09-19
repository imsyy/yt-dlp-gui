import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { save } from "@tauri-apps/plugin-dialog";
import { useI18n } from "vue-i18n";
import { showErrorDialog, formatError, sanitizeFilename } from "@/utils/format";
import { useSettingStore } from "@/stores/setting";
import { useVideoStore } from "@/stores/video";
import type {
  ChannelRecord,
  ChannelSyncProgressPayload,
  ChannelEnrichProgressPayload,
  ChannelVideoRecord,
  ChannelVideosPage,
  ChannelVideosQuery,
} from "@/types";

/** 归档视频的类型筛选 */
export type ChannelVideoType = "video" | "short" | "stream";
/** 归档视频的排序字段 */
export type ChannelSortBy = "published_at" | "view_count" | "duration";
/** 归档视频的排序方向 */
export type ChannelSortOrder = "asc" | "desc";
/** 同步模式：增量只拉取新内容，全量重新遍历所有分区 */
export type ChannelSyncMode = "incremental" | "full";
/** 归档列表的导出格式 */
export type ChannelExportFormat = "json" | "csv";

/** 单次请求的最大条数，与后端 channel_get_videos 的 page_size 上限保持一致 */
const PAGE_FETCH_SIZE = 1000;

/** 转义单个 CSV 字段 */
const csvCell = (value: string | number | null | undefined): string => {
  if (value === null || value === undefined) return "";
  return `"${String(value).replace(/"/g, '""')}"`;
};

/** 将归档视频序列化为 CSV 文本 */
const buildCsv = (items: ChannelVideoRecord[]): string => {
  const header = [
    "id",
    "title",
    "url",
    "contentType",
    "duration",
    "viewCount",
    "publishedAt",
    "publishedAccuracy",
  ];
  const rows = items.map((item) =>
    [
      csvCell(item.videoId),
      csvCell(item.title),
      csvCell(item.url),
      csvCell(item.contentType),
      csvCell(item.duration),
      csvCell(item.viewCount),
      csvCell(item.publishedAt ? new Date(item.publishedAt).toISOString() : null),
      csvCell(item.publishedAccuracy ?? ""),
    ].join(","),
  );
  return [header.join(","), ...rows].join("\r\n");
};

/**
 * 频道归档的数据与同步逻辑：频道列表、归档视频查询、后台同步任务与导出。
 * 页面只负责把这些状态分发给展示组件。
 */
export const useChannelArchive = () => {
  const { t } = useI18n();
  const settingStore = useSettingStore();
  const videoStore = useVideoStore();

  const channels = ref<ChannelRecord[]>([]);
  const channelsLoading = ref(false);
  const activeChannelId = ref<string | null>(null);
  const activeChannel = computed(
    () => channels.value.find((channel) => channel.id === activeChannelId.value) ?? null,
  );

  const videos = ref<ChannelVideoRecord[]>([]);
  const videosLoading = ref(false);
  const total = ref(0);
  const contentType = ref<ChannelVideoType>("video");
  const searchQuery = ref("");
  const debouncedSearch = refDebounced(searchQuery, 300);
  const sortBy = ref<ChannelSortBy>("published_at");
  const sortOrder = ref<ChannelSortOrder>("desc");

  const syncTabs = ref<string[]>(["videos", "shorts", "streams"]);
  const sleepInterval = ref(0.5);
  const syncProgress = ref<Record<string, ChannelSyncProgressPayload>>({});
  const enrichProgress = ref<Record<string, ChannelEnrichProgressPayload>>({});

  /** 组装一次归档视频查询，列表与导出共用同一组筛选条件 */
  const buildQuery = (channelId: string, page: number): ChannelVideosQuery => ({
    channelId,
    contentType: contentType.value,
    query: debouncedSearch.value.trim() || undefined,
    sortBy: sortBy.value,
    sortOrder: sortOrder.value,
    page,
    pageSize: PAGE_FETCH_SIZE,
  });

  /** 拉取当前频道的全部归档视频，虚拟列表负责渲染 */
  const loadVideos = async () => {
    const channelId = activeChannelId.value;
    if (!channelId) {
      videos.value = [];
      total.value = 0;
      return;
    }

    videosLoading.value = true;
    try {
      const items: ChannelVideoRecord[] = [];
      let nextPage = 1;
      while (true) {
        const result = await invoke<ChannelVideosPage>("channel_get_videos", {
          query: buildQuery(channelId, nextPage),
        });
        items.push(...result.items);
        total.value = result.total;
        // 单次请求有条数上限，按总数翻页直到取完
        if (result.items.length === 0 || items.length >= result.total) break;
        nextPage += 1;
      }
      // 请求期间可能已切换频道，丢弃过期结果
      if (activeChannelId.value !== channelId) return;
      videos.value = items;
    } catch (error: unknown) {
      showErrorDialog(String(error));
    } finally {
      videosLoading.value = false;
    }
  };

  /** 读取归档频道列表，并保证当前选中项始终有效 */
  const loadChannels = async () => {
    channelsLoading.value = true;
    try {
      const list = await invoke<ChannelRecord[]>("channel_list");
      channels.value = list;
      if (!list.some((channel) => channel.id === activeChannelId.value)) {
        activeChannelId.value = list[0]?.id ?? null;
      }
    } catch (error: unknown) {
      showErrorDialog(String(error));
    } finally {
      channelsLoading.value = false;
    }
  };

  /** 切换当前频道 */
  const selectChannel = (channelId: string) => {
    activeChannelId.value = channelId;
  };

  /** 新增频道成功后选中它并立即开始增量同步 */
  const handleChannelAdded = async (channel: ChannelRecord) => {
    await loadChannels();
    selectChannel(channel.id);
    await startSync("incremental", channel.id);
  };

  /** 启动后台同步任务 */
  const startSync = async (mode: ChannelSyncMode, channelId?: string) => {
    const targetId = channelId ?? activeChannelId.value;
    if (!targetId) return;

    const channel = channels.value.find((item) => item.id === targetId);
    if (channel) channel.syncStatus = "syncing";

    try {
      const { cookieFile, cookieBrowser } = await videoStore.getCookieArgs();
      await invoke("channel_sync_start", {
        channelId: targetId,
        mode,
        tabs: syncTabs.value,
        sleepInterval: sleepInterval.value,
        cookieFile,
        cookieBrowser,
        proxy: settingStore.proxy || null,
      });
    } catch (error: unknown) {
      if (channel) channel.syncStatus = "idle";
      showErrorDialog(String(error));
    }
  };

  /** 取消进行中的同步任务 */
  const cancelSync = async (channelId: string) => {
    try {
      await invoke("channel_sync_cancel", { channelId });
      const channel = channels.value.find((item) => item.id === channelId);
      if (channel) channel.syncStatus = "idle";
    } catch (error: unknown) {
      showErrorDialog(String(error));
    }
  };

  /** 手动查漏补缺：重抓本频道所有仍是模糊日期的视频（自动链路只跑一遍） */
  const startEnrich = async (channelId?: string) => {
    const targetId = channelId ?? activeChannelId.value;
    if (!targetId) return;

    try {
      const { cookieFile, cookieBrowser } = await videoStore.getCookieArgs();
      const total = await invoke<number>("channel_enrich_start", {
        channelId: targetId,
        videoIds: null,
        cookieFile,
        cookieBrowser,
        proxy: settingStore.proxy || null,
        concurrency: 6,
      });
      if (total === 0) {
        window.$message.success(t("channelArchive.enrichNothingToDo"));
      }
    } catch (error: unknown) {
      showErrorDialog(String(error));
    }
  };

  /** 取消进行中的日期校准任务（同步成功后自动链式启动的后台任务） */
  const cancelEnrich = async (channelId: string) => {
    try {
      await invoke("channel_enrich_cancel", { channelId });
    } catch (error: unknown) {
      showErrorDialog(String(error));
    }
  };

  /** 删除频道及其归档视频 */
  const removeChannel = async (channelId: string) => {
    try {
      await invoke("channel_delete", { channelId });
      delete syncProgress.value[channelId];
      delete enrichProgress.value[channelId];
      window.$message.success(t("channelArchive.deleteSuccess"));
      await loadChannels();
    } catch (error: unknown) {
      showErrorDialog(String(error));
    }
  };

  /** 导出当前已加载的归档视频（列表本身已按筛选条件全量加载） */
  const exportVideos = async (format: ChannelExportFormat) => {
    const channel = activeChannel.value;
    if (!channel || videos.value.length === 0) return;

    try {
      const filePath = await save({
        title: t("channelArchive.exportVideos"),
        defaultPath: `${sanitizeFilename(channel.title)}_videos.${format}`,
        filters: [{ name: format.toUpperCase(), extensions: [format] }],
      });
      if (!filePath) return;

      const items = videos.value;
      const content = format === "json" ? JSON.stringify(items, null, 2) : buildCsv(items);
      await invoke("tool_save_text_to_file", { filePath, content });
      window.$message.success(t("channelArchive.exportSuccess", { count: items.length }));
    } catch (error: unknown) {
      showErrorDialog(String(error));
    }
  };

  /** 应用后台同步进度事件（进度明细供详情区按类型展示） */
  const applyProgress = (payload: ChannelSyncProgressPayload) => {
    syncProgress.value[payload.channelId] = payload;

    const channel = channels.value.find((item) => item.id === payload.channelId);
    if (channel) {
      channel.syncStatus = payload.status === "syncing" ? "syncing" : "idle";
      if (payload.status === "completed") channel.videoCount = payload.totalSynced;
    }

    if (payload.status === "completed") {
      window.$message.success(
        t("channelArchive.syncSuccess", {
          newCount: payload.newSynced,
          totalCount: payload.totalSynced,
        }),
      );
      void loadChannels();
      if (activeChannelId.value === payload.channelId) void loadVideos();
    } else if (payload.status === "error") {
      window.$message.error(
        payload.message
          ? t("channelArchive.syncError", { error: payload.message })
          : t("channelArchive.syncError", { error: t("common.unknown") }),
      );
      void loadChannels();
    } else if (payload.status === "cancelled") {
      window.$message.info(t("channelArchive.syncCancelled"));
      void loadChannels();
    }
  };

  /** 应用后台日期校准进度事件 */
  const applyEnrichProgress = (payload: ChannelEnrichProgressPayload) => {
    enrichProgress.value[payload.channelId] = payload;

    if (payload.status === "completed") {
      window.$message.success(
        t("channelArchive.enrichSuccess", {
          fixed: payload.fixed,
          total: payload.total,
        }),
      );
      if (activeChannelId.value === payload.channelId) void loadVideos();
    } else if (payload.status === "error") {
      // 后端 message 可能是 err_ 错误码或 yt-dlp 原生 stderr 行，走 formatError 统一友好化，
      // 避免右下角弹出裸错误码
      const reason = payload.message ? formatError(payload.message) : t("common.unknown");
      window.$message.error(t("channelArchive.enrichError", { error: reason }));
    } else if (payload.status === "cancelled") {
      window.$message.info(t("channelArchive.enrichCancelled"));
      if (activeChannelId.value === payload.channelId) void loadVideos();
    }
  };

  // 频道或筛选条件变化时重新加载归档视频
  watch([activeChannelId, debouncedSearch, contentType, sortBy, sortOrder], () => {
    void loadVideos();
  });

  let unlistenProgress: UnlistenFn | null = null;
  let unlistenEnrich: UnlistenFn | null = null;

  onMounted(async () => {
    unlistenProgress = await listen<ChannelSyncProgressPayload>("channel-sync-progress", (event) =>
      applyProgress(event.payload),
    );
    unlistenEnrich = await listen<ChannelEnrichProgressPayload>(
      "channel-enrich-progress",
      (event) => applyEnrichProgress(event.payload),
    );
    await loadChannels();
    // 重进页面时恢复仍在跑的校准任务状态（事件监听在切页时已注销，靠后端在跑任务表补种）
    try {
      const active = await invoke<string[]>("channel_enrich_active");
      for (const id of active) {
        const current = enrichProgress.value[id];
        if (!current || current.status !== "enriching") {
          enrichProgress.value[id] = {
            channelId: id,
            status: "enriching",
            total: 0,
            done: 0,
            fixed: 0,
            message: null,
          };
        }
      }
    } catch {
      // 查询失败不影响列表展示
    }
  });

  onUnmounted(() => {
    unlistenProgress?.();
    unlistenProgress = null;
    unlistenEnrich?.();
    unlistenEnrich = null;
  });

  return {
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
    enrichProgress,
    selectChannel,
    handleChannelAdded,
    startSync,
    cancelSync,
    startEnrich,
    cancelEnrich,
    removeChannel,
    exportVideos,
    loadVideos,
  };
};
