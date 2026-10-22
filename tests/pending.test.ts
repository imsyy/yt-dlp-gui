import { beforeEach, expect, test, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { useSettingStore } from "@/stores/setting";
import { createPendingItem, usePendingStore } from "@/stores/pending";
import type { FetchedVideoData, VideoFormat } from "@/types";

vi.mock("@/locales", () => ({ resolveLocale: () => "en-US", setI18nLocale: vi.fn() }));

const data: FetchedVideoData = {
  url: "https://example.com/video",
  videoInfo: {
    title: "Video",
    thumbnail: "",
    duration: 60,
    uploader: "",
    view_count: 0,
    upload_date: "",
    description: "",
    formats: [],
    subtitles: {},
    automatic_captions: {},
  },
  videoFormats: [],
  audioFormats: [],
  isPlaylist: false,
  playlistEntries: [],
};

beforeEach(() => setActivePinia(createPinia()));

test("new standard tasks use the configured default mode and can reset to default", () => {
  const settings = useSettingStore();
  expect(createPendingItem(data).downloadMode).toBe("default");
  settings.defaultDownloadMode = "audio";
  const existing = createPendingItem(data);
  expect(existing.downloadMode).toBe("audio");
  settings.defaultDownloadMode = "video";
  expect(createPendingItem(data).downloadMode).toBe("video");
  settings.defaultDownloadMode = "default";
  expect(createPendingItem(data).downloadMode).toBe("default");
  expect(existing.downloadMode).toBe("audio");
});

test("quick tasks retain their separate download preset", () => {
  const settings = useSettingStore();
  settings.defaultDownloadMode = "audio";
  settings.quickDownloadMode = "video";
  expect(createPendingItem(data, true).downloadMode).toBe("video");
});

test("new playlist tasks select extractor indices including gaps", () => {
  const item = createPendingItem({
    ...data,
    isPlaylist: true,
    playlistEntries: [
      { id: "two", title: "Second", duration: 60, url: "", playlist_index: 2 },
      { id: "five", title: "Fifth", duration: 60, url: "", playlist_index: 5 },
    ],
  });
  expect(item.selectedPlaylistItems).toEqual([2, 5]);
});

const format = (id: string): VideoFormat => ({
  format_id: id,
  ext: "mp4",
  resolution: "",
  height: null,
  width: null,
  fps: null,
  vcodec: "none",
  acodec: "none",
  filesize: null,
  filesize_approx: null,
  format_note: "",
  tbr: null,
  abr: null,
});

test("refresh retains selected formats and playlist entries when they remain available", () => {
  const store = usePendingStore();
  const refreshed = {
    ...data,
    videoFormats: [format("v1"), format("v2")],
    audioFormats: [format("a1"), format("a2"), format("a3")],
    isPlaylist: true,
    playlistEntries: [2, 5, 7].map((index) => ({
      id: String(index),
      title: String(index),
      url: "",
      duration: 60,
      playlist_index: index,
    })),
  };
  const id = store.add(refreshed);
  const item = store.activeItem!;
  item.selectedVideoFormat = "v2";
  item.selectedAudioFormat = "a2+a3";
  item.selectedPlaylistItems = [5];
  item.customArgs = "--sleep-requests 2";
  store.refresh(id, refreshed);
  expect(item.selectedVideoFormat).toBe("v2");
  expect(item.selectedAudioFormat).toBe("a2+a3");
  expect(item.selectedPlaylistItems).toEqual([5]);
  expect(item.customArgs).toBe("--sleep-requests 2");

  store.refresh(id, { ...refreshed, videoFormats: [format("v3")], audioFormats: [format("a3")] });
  expect(item.selectedVideoFormat).toBe("v3");
  expect(item.selectedAudioFormat).toBe("a3");
});

test("refresh does not silently select other episodes when selected ones disappear", () => {
  const store = usePendingStore();
  const id = store.add({
    ...data,
    isPlaylist: true,
    playlistEntries: [{ id: "2", title: "Two", url: "", duration: 60, playlist_index: 2 }],
  });
  store.refresh(id, {
    ...data,
    isPlaylist: true,
    playlistEntries: [{ id: "5", title: "Five", url: "", duration: 60, playlist_index: 5 }],
  });
  expect(store.activeItem!.selectedPlaylistItems).toEqual([]);
});
