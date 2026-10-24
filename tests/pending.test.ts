import { beforeEach, expect, test, vi } from "vitest";
import { createPinia, disposePinia, setActivePinia } from "pinia";
import { createPersistedState } from "pinia-plugin-persistedstate";
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
  expect(createPendingItem(data).forceKeyframesAtCuts).toBe(false);
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

const codecData = (): FetchedVideoData => ({
  ...data,
  videoFormats: [
    { ...format("av1-2160"), vcodec: "av01.0.12M.08", height: 2160 },
    { ...format("vp9-1080"), vcodec: "vp09.00.40.08", height: 1080 },
    { ...format("h264-720"), vcodec: "avc1.64001f", height: 720 },
  ],
  audioFormats: [
    { ...format("opus"), acodec: "opus" },
    { ...format("aac"), acodec: "mp4a.40.2" },
  ],
});

test("new tasks apply normalized video and audio codec preferences", () => {
  const settings = useSettingStore();
  settings.preferredVideoCodec = "h264";
  settings.preferredAudioCodec = "aac";
  const item = createPendingItem(codecData());
  expect(item.selectedVideoFormat).toBe("h264-720");
  expect(item.selectedAudioFormat).toBe("aac");
});

test("quick height limits take priority over the saved codec", () => {
  const settings = useSettingStore();
  settings.preferredVideoCodec = "av1";
  settings.quickMaxHeight = 1080;
  expect(createPendingItem(codecData(), true).selectedVideoFormat).toBe("vp9-1080");
  settings.preferredVideoCodec = "h264";
  expect(createPendingItem(codecData(), true).selectedVideoFormat).toBe("h264-720");
});

test("unsupported codec preferences fall back without clearing the preference", () => {
  const settings = useSettingStore();
  settings.preferredVideoCodec = "hevc";
  settings.preferredAudioCodec = "vorbis";
  const item = createPendingItem(codecData());
  expect(item.selectedVideoFormat).toBe("av1-2160");
  expect(item.selectedAudioFormat).toBe("opus");
  expect(settings.preferredVideoCodec).toBe("hevc");
  expect(settings.preferredAudioCodec).toBe("vorbis");
  settings.preferredVideoCodec = "all";
  settings.preferredAudioCodec = "all";
  expect(createPendingItem(codecData()).selectedVideoFormat).toBe("av1-2160");
});

test("changed global codec preferences never reset an existing pending item or refresh", () => {
  const store = usePendingStore();
  const settings = useSettingStore();
  const source = codecData();
  const id = store.add(source);
  store.activeItem!.selectedVideoFormat = "vp9-1080";
  store.activeItem!.selectedAudioFormat = "opus+aac";
  settings.preferredVideoCodec = "h264";
  settings.preferredAudioCodec = "aac";
  expect(store.activeItem!.selectedVideoFormat).toBe("vp9-1080");
  store.refresh(id, { ...source, videoInfo: { ...source.videoInfo } });
  expect(store.activeItem!.selectedVideoFormat).toBe("vp9-1080");
  expect(store.activeItem!.selectedAudioFormat).toBe("opus+aac");
  expect(createPendingItem(source).selectedVideoFormat).toBe("h264-720");
});

test("codec preferences restore through the existing setting persistence", () => {
  const entries = new Map<string, string>();
  const storage = {
    getItem: (key: string) => entries.get(key) ?? null,
    setItem: (key: string, value: string) => {
      entries.set(key, value);
    },
  };
  const first = createPinia().use(createPersistedState({ storage }));
  createApp({}).use(first);
  const settings = useSettingStore(first);
  settings.preferredVideoCodec = "h264";
  settings.preferredAudioCodec = "aac";
  settings.$persist();
  disposePinia(first);

  const restored = createPinia().use(createPersistedState({ storage }));
  createApp({}).use(restored);
  expect(useSettingStore(restored).preferredVideoCodec).toBe("h264");
  expect(useSettingStore(restored).preferredAudioCodec).toBe("aac");
  disposePinia(restored);
});
