import { beforeEach, expect, test, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { useDownloadLauncher } from "@/composables/useDownloadLauncher";
import type { PendingItem } from "@/types";

const { status, addTask } = vi.hoisted(() => ({
  status: { showFfmpegSetupModal: false },
  addTask: vi.fn(),
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("vue-i18n", () => ({ useI18n: () => ({ t: (key: string) => key }) }));
vi.mock("@/stores/setting", () => ({ useSettingStore: () => ({ downloadDir: "C:/Downloads" }) }));
vi.mock("@/stores/status", () => ({ useStatusStore: () => status }));
vi.mock("@/stores/download", () => ({ useDownloadStore: () => ({ addTask }) }));
vi.mock("@/stores/video", () => ({ useVideoStore: () => ({ getCookieArgs: vi.fn() }) }));

beforeEach(() => {
  vi.clearAllMocks();
  status.showFfmpegSetupModal = false;
  vi.mocked(invoke).mockResolvedValue({ installed: true, runnable: false });
});

const processingOptions: Partial<PendingItem>[] = [
  { downloadMode: "default", selectedVideoFormat: "137", selectedAudioFormat: "140" },
  { downloadMode: "default", selectedVideoFormat: "137" },
  { downloadMode: "audio", extractAudio: true },
  { downloadMode: "video", startTime: new Date(2026, 0, 1, 0, 0, 10).getTime() },
  { downloadMode: "video", remuxFormat: "mkv" },
];

test.each(processingOptions)(
  "rejects installed but unusable FFmpeg for $downloadMode processing",
  async (options) => {
    const item: PendingItem = {
      id: "test",
      createdAt: 0,
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
      selectedPlaylistItems: [],
      downloadMode: "default",
      selectedVideoFormat: "",
      selectedAudioFormat: "",
      startTime: null,
      endTime: null,
      embedSubs: false,
      embedThumbnail: false,
      writeThumbnail: false,
      writeDescription: false,
      embedMetadata: false,
      embedChapters: false,
      sponsorblockRemove: false,
      extractAudio: false,
      audioConvertFormat: "",
      noMerge: false,
      recodeFormat: "",
      remuxFormat: "",
      limitRate: "",
      ffmpegArgs: "",
      customArgs: "",
      selectedSubtitles: [],
      liveFromStart: false,
      ...options,
    };
    const result = await useDownloadLauncher().launchDownload(item);
    expect(result).toBe("missing-ffmpeg");
    expect(status.showFfmpegSetupModal).toBe(true);
    expect(invoke).toHaveBeenCalledExactlyOnceWith("get_ffmpeg_status");
    expect(addTask).not.toHaveBeenCalled();
  },
);
