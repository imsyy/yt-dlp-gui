import { defineStore } from "pinia";
import { useSettingStore } from "@/stores/setting";
import type { FetchedVideoData, PendingItem } from "@/types";

const generateId = () => `pd_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`;

/** 额外选项默认值取自 settingStore，即上次修改后记住的选择 */
const createItem = (data: FetchedVideoData): PendingItem => {
  const settingStore = useSettingStore();
  return {
    ...data,
    id: generateId(),
    createdAt: Date.now(),
    selectedPlaylistItems: data.isPlaylist ? data.playlistEntries.map((_, i) => i + 1) : [],
    downloadMode: "default",
    selectedVideoFormat: data.videoFormats[0]?.format_id ?? "",
    selectedAudioFormat: data.audioFormats[0]?.format_id ?? "",
    startTime: null,
    endTime: null,
    embedSubs: settingStore.embedSubs,
    embedThumbnail: settingStore.embedThumbnail,
    embedMetadata: settingStore.embedMetadata,
    embedChapters: settingStore.embedChapters,
    sponsorblockRemove: settingStore.sponsorblockRemove,
    extractAudio: settingStore.extractAudio,
    audioConvertFormat: settingStore.audioConvertFormat,
    noMerge: settingStore.noMerge,
    recodeFormat: settingStore.recodeFormat,
    limitRate: settingStore.limitRate,
    ffmpegArgs: settingStore.ffmpegArgs,
    selectedSubtitles: [],
  };
};

export const usePendingStore = defineStore("pending", () => {
  const items = ref<PendingItem[]>([]);
  const activeId = ref<string>("");

  const activeItem = computed<PendingItem | null>(
    () => items.value.find((i) => i.id === activeId.value) ?? null,
  );

  const add = (data: FetchedVideoData): string => {
    const item = createItem(data);
    items.value.push(item);
    activeId.value = item.id;
    return item.id;
  };

  const remove = (id: string) => {
    const idx = items.value.findIndex((i) => i.id === id);
    if (idx === -1) return;
    items.value.splice(idx, 1);
    if (activeId.value === id) {
      const next = items.value[idx] ?? items.value[idx - 1] ?? items.value[0];
      activeId.value = next ? next.id : "";
    }
  };

  /** 刷新当前项：替换源数据并重置依赖源数据的派生字段（格式/分P 选中），保留用户填的额外选项 */
  const refresh = (id: string, data: FetchedVideoData) => {
    const item = items.value.find((i) => i.id === id);
    if (!item) return;
    item.url = data.url;
    item.videoInfo = data.videoInfo;
    item.videoFormats = data.videoFormats;
    item.audioFormats = data.audioFormats;
    item.isPlaylist = data.isPlaylist;
    item.playlistEntries = data.playlistEntries;
    item.selectedPlaylistItems = data.isPlaylist
      ? data.playlistEntries.map((_, i) => i + 1)
      : [];
    item.selectedVideoFormat = data.videoFormats[0]?.format_id ?? "";
    item.selectedAudioFormat = data.audioFormats[0]?.format_id ?? "";
  };

  const clear = () => {
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
