import { defineStore } from "pinia";
import { useSettingStore } from "@/stores/setting";
import type { FetchedVideoData, PendingItem, VideoFormat } from "@/types";

const generateId = () => `pd_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`;

const selectVideoFormat = (formats: VideoFormat[], maxHeight?: number): string => {
  if (!maxHeight) return formats[0]?.format_id ?? "";
  return (
    formats.find((format) => format.height != null && format.height <= maxHeight)?.format_id ??
    formats[0]?.format_id ??
    ""
  );
};

/**
 * 根据解析后的视频元数据创建新的待下载任务配置对象
 *
 * @param data 解析完成的视频数据
 * @param quick 是否采用快速下载预设配置
 * @returns 初始化后的待下载项
 */
export const createPendingItem = (data: FetchedVideoData, quick = false): PendingItem => {
  const settingStore = useSettingStore();
  const maxHeight = quick ? settingStore.quickMaxHeight : undefined;
  return {
    ...data,
    id: generateId(),
    createdAt: Date.now(),
    selectedPlaylistItems: data.isPlaylist ? data.playlistEntries.map((_, i) => i + 1) : [],
    downloadMode: quick ? settingStore.quickDownloadMode : "default",
    selectedVideoFormat: selectVideoFormat(data.videoFormats, maxHeight),
    selectedAudioFormat: data.audioFormats[0]?.format_id ?? "",
    startTime: null,
    endTime: null,
    embedSubs: quick ? false : settingStore.defaultEmbedSubs,
    embedThumbnail: quick ? settingStore.quickEmbedThumbnail : settingStore.defaultEmbedThumbnail,
    writeThumbnail: quick ? settingStore.quickWriteThumbnail : settingStore.defaultWriteThumbnail,
    writeDescription: quick
      ? settingStore.quickWriteDescription
      : settingStore.defaultWriteDescription,
    embedMetadata: quick ? settingStore.quickEmbedMetadata : settingStore.defaultEmbedMetadata,
    embedChapters: quick ? settingStore.quickEmbedChapters : settingStore.defaultEmbedChapters,
    sponsorblockRemove: quick
      ? settingStore.quickSponsorblockRemove
      : settingStore.defaultSponsorblockRemove,
    extractAudio: quick ? false : settingStore.defaultExtractAudio,
    audioConvertFormat: quick ? "" : settingStore.defaultAudioConvertFormat,
    noMerge: quick ? settingStore.quickNoMerge : settingStore.defaultNoMerge,
    recodeFormat: quick ? settingStore.quickRecodeFormat : settingStore.defaultRecodeFormat,
    remuxFormat: quick ? settingStore.quickRemuxFormat : settingStore.defaultRemuxFormat,
    limitRate: quick ? settingStore.quickLimitRate : settingStore.defaultLimitRate,
    ffmpegArgs: quick ? settingStore.quickFfmpegArgs : settingStore.defaultFfmpegArgs,
    customArgs: quick ? settingStore.quickCustomArgs : settingStore.defaultCustomArgs,
    selectedSubtitles: [],
    liveFromStart: data.videoInfo.is_live === true || data.videoInfo.live_status === "is_live",
  };
};

/**
 * 待下载任务配置队列 Store
 *
 * 负责在正式开始下载前暂存已解析的视频项、用户参数微调与多任务切换。
 */
export const usePendingStore = defineStore("pending", () => {
  const items = ref<PendingItem[]>([]);
  const activeId = ref<string>("");

  const activeItem = computed<PendingItem | null>(
    () => items.value.find((i) => i.id === activeId.value) ?? null,
  );

  /**
   * 将解析出的视频数据加入待下载项列表，并将其激活为当前编辑项
   *
   * @param data 解析出的视频数据
   * @returns 新增待下载项的唯一 ID
   */
  const add = (data: FetchedVideoData): string => {
    const item = createPendingItem(data);
    items.value.push(item);
    activeId.value = item.id;
    return item.id;
  };

  /**
   * 移除指定 ID 的待下载项，并自动激活临近项
   *
   * @param id 待删除项 ID
   */
  const remove = (id: string): void => {
    const idx = items.value.findIndex((i) => i.id === id);
    if (idx === -1) return;
    items.value.splice(idx, 1);
    if (activeId.value === id) {
      const next = items.value[idx] ?? items.value[idx - 1] ?? items.value[0];
      activeId.value = next ? next.id : "";
    }
  };

  /**
   * 刷新当前待下载项的元数据（重新解析后覆盖音视频流信息，保留用户已修改的选项）
   *
   * @param id 待下载项 ID
   * @param data 重新解析获得的视频数据
   */
  const refresh = (id: string, data: FetchedVideoData): void => {
    const item = items.value.find((i) => i.id === id);
    if (!item) return;
    item.url = data.url;
    item.videoInfo = data.videoInfo;
    item.videoFormats = data.videoFormats;
    item.audioFormats = data.audioFormats;
    item.isPlaylist = data.isPlaylist;
    item.playlistEntries = data.playlistEntries;
    item.selectedPlaylistItems = data.isPlaylist ? data.playlistEntries.map((_, i) => i + 1) : [];
    item.selectedVideoFormat = data.videoFormats[0]?.format_id ?? "";
    item.selectedAudioFormat = data.audioFormats[0]?.format_id ?? "";
  };

  /**
   * 清空全部待下载项
   */
  const clear = (): void => {
    items.value = [];
    activeId.value = "";
  };

  return {
    items,
    activeId,
    activeItem,
    add,
    remove,
    refresh,
    clear,
  };
});
