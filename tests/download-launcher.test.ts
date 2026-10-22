import { beforeEach, expect, test, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { useDownloadLauncher } from "@/composables/useDownloadLauncher";
import type { PendingItem } from "@/types";

const { status, addTask, tasks } = vi.hoisted(() => ({
  status: { showFfmpegSetupModal: false },
  addTask: vi.fn(),
  tasks: [] as { id: string; status: string; error?: string }[],
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("vue-i18n", () => ({ useI18n: () => ({ t: (key: string) => key }) }));
vi.mock("@/stores/setting", () => ({
  useSettingStore: () => ({
    downloadDir: "C:/Downloads",
    outputTemplate: "%(title)s.%(ext)s",
    filenamePrefix: "",
    filenameSuffix: "",
  }),
}));
vi.mock("@/stores/status", () => ({ useStatusStore: () => status }));
vi.mock("@/stores/download", () => ({
  useDownloadStore: () => ({ addTask, tasks }),
}));
vi.mock("@/stores/video", () => ({
  useVideoStore: () => ({ getCookieArgs: async () => ({ cookieFile: null, cookieBrowser: null }) }),
}));

beforeEach(() => {
  vi.clearAllMocks();
  addTask.mockReset();
  addTask.mockResolvedValue("started");
  tasks.splice(0);
  status.showFfmpegSetupModal = false;
  vi.mocked(invoke).mockResolvedValue({ installed: true, runnable: false });
});

const processingOptions: Partial<PendingItem>[] = [
  { downloadMode: "default", selectedVideoFormat: "137", selectedAudioFormat: "140" },
  { downloadMode: "default", selectedVideoFormat: "137" },
  { downloadMode: "default", selectedAudioFormat: "en+es" },
  { downloadMode: "audio", extractAudio: true },
  { downloadMode: "video", startTime: new Date(2026, 0, 1, 0, 0, 10).getTime() },
  { downloadMode: "video", remuxFormat: "mkv" },
];

const createItem = (options: Partial<PendingItem> = {}): PendingItem => ({
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
});

test.each(processingOptions)(
  "rejects installed but unusable FFmpeg for $downloadMode processing",
  async (options) => {
    const item = createItem(options);
    const result = await useDownloadLauncher().launchDownload(item);
    expect(result).toBe("missing-ffmpeg");
    expect(status.showFfmpegSetupModal).toBe(true);
    expect(invoke).toHaveBeenCalledExactlyOnceWith("get_ffmpeg_status");
    expect(addTask).not.toHaveBeenCalled();
  },
);

test("downloads an existing combined file without requiring FFmpeg", async () => {
  const item = createItem({
    selectedVideoFormat: "combined",
    videoFormats: [
      {
        format_id: "combined",
        ext: "mp4",
        resolution: "720p",
        height: 720,
        width: 1280,
        fps: 30,
        vcodec: "h264",
        acodec: "aac",
        filesize: null,
        filesize_approx: null,
        format_note: "",
        tbr: null,
        abr: null,
      },
    ],
  });
  expect(await useDownloadLauncher().launchDownload(item)).toBe("started");
  expect(invoke).not.toHaveBeenCalledWith("get_ffmpeg_status");
  expect(addTask).toHaveBeenCalledWith(
    expect.objectContaining({
      status: "queued",
      params: expect.objectContaining({ videoHasAudio: true }),
    }),
    undefined,
  );
});

test("downloads audio-only sources in default mode without requiring FFmpeg", async () => {
  const item = createItem({ selectedAudioFormat: "audio-only" });
  expect(await useDownloadLauncher().launchDownload(item)).toBe("started");
  expect(invoke).not.toHaveBeenCalledWith("get_ffmpeg_status");
  expect(addTask).toHaveBeenCalledWith(
    expect.objectContaining({
      params: expect.objectContaining({ videoFormat: null, audioFormat: "audio-only" }),
    }),
    undefined,
  );
});

test("separate audio tracks bypass FFmpeg when merging is disabled", async () => {
  expect(
    await useDownloadLauncher().launchDownload(
      createItem({ selectedAudioFormat: "en+es", noMerge: true }),
    ),
  ).toBe("started");
  expect(invoke).not.toHaveBeenCalledWith("get_ffmpeg_status");
});

test("the launcher delegates queued submissions and never starts a second process itself", async () => {
  addTask.mockResolvedValue("queued");
  const result = await useDownloadLauncher().launchDownload(createItem());
  expect(result).toBe("queued");
  expect(addTask).toHaveBeenCalledExactlyOnceWith(
    expect.objectContaining({ status: "queued" }),
    undefined,
  );
  expect(invoke).not.toHaveBeenCalledWith("start_download", expect.anything());
});

test("preparing task replacement uses the same persistence and scheduling entry", async () => {
  expect(await useDownloadLauncher().launchDownload(createItem(), "preparing")).toBe("started");
  expect(addTask).toHaveBeenCalledWith(expect.objectContaining({ status: "queued" }), "preparing");
  expect(invoke).not.toHaveBeenCalledWith("db_delete_task", expect.anything());
});

test("a store startup failure remains available for diagnostics and is shown to the user", async () => {
  const message = { error: vi.fn() };
  vi.stubGlobal("window", { $message: message });
  addTask.mockImplementation(async (task: { id: string }) => {
    tasks.push({ id: task.id, status: "error", error: "cannot start download" });
    return "failed";
  });
  try {
    expect(await useDownloadLauncher().launchDownload(createItem())).toBe("failed");
    expect(message.error).toHaveBeenCalledWith("cannot start download");
    expect(tasks).toHaveLength(1);
    expect(invoke).not.toHaveBeenCalledWith("db_delete_task", expect.anything());
  } finally {
    vi.unstubAllGlobals();
  }
});
