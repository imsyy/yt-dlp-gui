import { beforeEach, expect, test, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { useSettingStore } from "@/stores/setting";
import { createPendingItem } from "@/stores/pending";
import type { FetchedVideoData } from "@/types";

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
