import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { createRenderer, getCurrentInstance, h, ssrContextKey } from "vue";
import type { Component, ComponentInternalInstance } from "vue";
import TaskEditModal from "@/components/downloads/TaskEditModal.vue";
import { normalizeTaskParams } from "@/utils/taskParams";
import type { DownloadTask, DownloadTaskParams } from "@/types";

const { retryTask, message } = vi.hoisted(() => ({
  retryTask: vi.fn(),
  message: { error: vi.fn() },
}));
vi.mock("vue-i18n", () => ({ useI18n: () => ({ t: (key: string) => key }) }));
vi.mock("@/stores/download", () => ({ useDownloadStore: () => ({ retryTask }) }));
vi.mock("@/utils/format", () => ({ formatError: (text: string) => text }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));

const renderer = createRenderer<Record<string, never>, Record<string, never>>({
  createElement: () => ({}),
  createText: () => ({}),
  createComment: () => ({}),
  setText: () => {},
  setElementText: () => {},
  patchProp: () => {},
  parentNode: () => null,
  nextSibling: () => null,
  insert: () => {},
  remove: () => {},
});

interface ModalState {
  draft: DownloadTaskParams;
  valid: boolean;
  handleSubmit: () => Promise<void>;
  updateVideoFormat: (value: string) => void;
}

const unmounts: (() => void)[] = [];
const mountEditor = () => {
  const task = reactive<DownloadTask>({
    id: "original",
    url: "https://example.com/video",
    title: "Video",
    thumbnail: "",
    formatLabel: "1080p",
    status: "error",
    percent: 0,
    speed: "",
    eta: "",
    downloaded: "",
    total: "",
    logs: ["original failure"],
    createdAt: 0,
    params: normalizeTaskParams({
      downloadDir: "D:/Downloads",
      cookieFile: "D:/cookies.txt",
      cookieBrowser: "firefox",
      proxy: "http://proxy:8080",
      outputTemplate: "original/%(title)s.%(ext)s",
      subtitles: ["sub:en"],
      playlistItems: "1,3",
      noPlaylist: false,
      videoFormat: "137",
      audioFormat: "140",
      customArgs: "--sleep-requests 2",
    }),
  });
  const state = reactive({ show: true });
  let modal!: ModalState;
  // Keep the real setup/model/watch behavior without a DOM or Naive UI rendering dependency.
  const Editor: Component = {
    ...TaskEditModal,
    render: () => {
      modal = (getCurrentInstance() as ComponentInternalInstance & { setupState: ModalState })
        .setupState;
      return null;
    },
  } as Component;
  const app = renderer.createApp({
    render: () =>
      h(Editor, {
        task,
        show: state.show,
        "onUpdate:show": (value: boolean) => {
          state.show = value;
        },
      }),
  });
  app.provide(ssrContextKey, { modules: new Set() });
  app.mount({});
  unmounts.push(() => app.unmount());
  return { task, state, modal };
};

beforeEach(() => {
  retryTask.mockReset();
  retryTask.mockResolvedValue(true);
  message.error.mockClear();
  vi.stubGlobal("window", { $message: message });
});
afterEach(() => {
  unmounts.splice(0).forEach((unmount) => unmount());
  vi.unstubAllGlobals();
});

test("editing and cancelling a draft leaves the original parameters and diagnostics intact", async () => {
  const { task, state, modal } = mountEditor();
  const original = JSON.parse(JSON.stringify(task));
  modal.draft.downloadDir = "D:/Changed";
  modal.draft.subtitles.push("sub:fr");
  modal.draft.customArgs = "--sleep-requests 10";
  state.show = false;
  await nextTick();
  expect(task).toEqual(original);
  expect(retryTask).not.toHaveBeenCalled();
});

test("draft validation accepts long clips without assuming a source duration", async () => {
  const { modal } = mountEditor();
  modal.draft.startTime = 90000;
  modal.draft.endTime = 90100;
  expect(modal.valid).toBe(true);
  modal.draft.endTime = 90000;
  expect(modal.valid).toBe(false);
  await modal.handleSubmit();
  expect(retryTask).not.toHaveBeenCalled();
  modal.draft.endTime = null;
  expect(modal.valid).toBe(true);
  modal.draft.outputTemplate = "broken%";
  expect(modal.valid).toBe(false);
  modal.draft.outputTemplate = null;
  modal.draft.downloadDir = " ";
  expect(modal.valid).toBe(false);
});

test("submission keeps the original authentication, subtitles and playlist selection", async () => {
  const { task, state, modal } = mountEditor();
  modal.draft.downloadMode = "audio";
  modal.draft.startTime = 10;
  modal.draft.endTime = 30;
  modal.draft.noMerge = true;
  modal.draft.forceKeyframesAtCuts = true;
  await modal.handleSubmit();
  expect(retryTask).toHaveBeenCalledExactlyOnceWith(
    "original",
    expect.objectContaining({
      downloadMode: "audio",
      startTime: 10,
      endTime: 30,
      noMerge: true,
      forceKeyframesAtCuts: true,
      cookieFile: "D:/cookies.txt",
      cookieBrowser: "firefox",
      proxy: "http://proxy:8080",
      outputTemplate: "original/%(title)s.%(ext)s",
      subtitles: ["sub:en"],
      playlistItems: "1,3",
      noPlaylist: false,
      customArgs: "--sleep-requests 2",
    }),
  );
  expect(state.show).toBe(false);
  expect(task.status).toBe("error");
  expect(task.params.downloadMode).toBe("default");
});

test("unsupported multiple-track extraction is visible and cannot be submitted", async () => {
  const { modal } = mountEditor();
  modal.draft.audioFormat = "en+es";
  expect(modal.valid).toBe(true);
  modal.draft.extractAudio = true;
  expect(modal.valid).toBe(false);
  await modal.handleSubmit();
  expect(retryTask).not.toHaveBeenCalled();
  modal.draft.audioFormat = "en";
  expect(modal.valid).toBe(true);
});

test("opening the editor again resets unsaved fields from the current task", async () => {
  const { task, state, modal } = mountEditor();
  modal.draft.downloadDir = "D:/Discard";
  state.show = false;
  await nextTick();
  task.params.downloadDir = "D:/Current";
  state.show = true;
  await nextTick();
  expect(modal.draft.downloadDir).toBe("D:/Current");
});

test("restoring the original combined format keeps its known audio capability", () => {
  const { task, modal } = mountEditor();
  task.params.videoHasAudio = true;
  modal.updateVideoFormat("different");
  expect(modal.draft.videoHasAudio).toBe(false);
  modal.updateVideoFormat("137");
  expect(modal.draft.videoHasAudio).toBe(true);
});
