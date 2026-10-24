import { beforeEach, expect, test, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { createRenderer, getCurrentInstance, h, ssrContextKey } from "vue";
import DownloadOptionsCard from "@/components/home/DownloadOptionsCard.vue";
import { useSettingStore } from "@/stores/setting";
import type { VideoFormat, VideoInfo } from "@/types";
import type { Component, ComponentInternalInstance } from "vue";

vi.mock("vue-i18n", () => ({ useI18n: () => ({ t: (key: string) => key }) }));
vi.mock("@/locales", () => ({ resolveLocale: () => "en-US", setI18nLocale: vi.fn() }));

beforeEach(() => setActivePinia(createPinia()));

// Exercise the component's real model/watch lifecycle without a browser or DOM dependency.
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

const format = (id: string, video = false): VideoFormat => ({
  format_id: id,
  ext: video ? "mp4" : "webm",
  resolution: "",
  height: video ? 1080 : null,
  width: null,
  fps: null,
  vcodec: video ? "h264" : "none",
  acodec: video ? "none" : "opus",
  filesize: null,
  filesize_approx: null,
  format_note: "",
  tbr: null,
  abr: null,
});

const videoInfo: VideoInfo = {
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
};

const mountCard = (mode: "default" | "video" | "audio" = "default") => {
  const state = reactive({
    downloadMode: mode,
    selectedVideoFormat: "video",
    selectedAudioFormat: "en+es",
    extractAudio: false,
    videoInfo: { ...videoInfo },
    videoFormats: [format("video", true)],
    audioFormats: [format("en"), format("es")],
  });
  let componentState: Record<string, unknown> = {};
  const Card: Component = {
    ...DownloadOptionsCard,
    render: () => {
      const instance = getCurrentInstance() as ComponentInternalInstance & {
        setupState: Record<string, unknown>;
      };
      componentState = instance.setupState;
      return null;
    },
  } as Component;
  const app = renderer.createApp({
    render: () =>
      h(Card, {
        ...state,
        "onUpdate:selectedVideoFormat": (value: string) => {
          state.selectedVideoFormat = value;
        },
        "onUpdate:selectedAudioFormat": (value: string) => {
          state.selectedAudioFormat = value;
        },
      }),
  });
  // Vitest transforms SFCs for SSR, whose setup registers this module in the context.
  app.provide(ssrContextKey, { modules: new Set() });
  app.mount({});
  return {
    state,
    multipleAudio: () => componentState.multipleAudio,
    videoCodec: () => componentState.selectedVideoCodec,
    audioCodec: () => componentState.selectedAudioCodec,
    visibleVideoFormats: () => componentState.filteredVideoFormats as VideoFormat[],
    visibleAudioFormats: () => componentState.filteredAudioFormats as VideoFormat[],
    selectVideoCodec: (value: string) =>
      (componentState.handleVideoCodecChange as (value: string) => void)(value),
    selectAudioCodec: (value: string) =>
      (componentState.handleAudioCodecChange as (value: string) => void)(value),
    unmount: () => app.unmount(),
  };
};

test("switching to video-only and back preserves all selected audio tracks", async () => {
  const card = mountCard();
  expect(card.multipleAudio()).toBe(true);
  card.state.downloadMode = "video";
  await nextTick();
  expect(card.state.selectedAudioFormat).toBe("en+es");
  card.state.downloadMode = "default";
  await nextTick();
  expect(card.state.selectedAudioFormat).toBe("en+es");
  expect(card.multipleAudio()).toBe(true);
  card.unmount();
});

test("switching from hidden audio to audio-only normalizes an unsupported multi-track value", async () => {
  const card = mountCard("video");
  expect(card.state.selectedAudioFormat).toBe("en+es");
  card.state.downloadMode = "audio";
  await nextTick();
  expect(card.state.selectedAudioFormat).toBe("en");
  expect(card.multipleAudio()).toBe(false);
  card.unmount();
});

test("audio extraction keeps one selected track", async () => {
  const card = mountCard();
  card.state.extractAudio = true;
  await nextTick();
  expect(card.state.selectedAudioFormat).toBe("en");
  expect(card.multipleAudio()).toBe(false);
  card.unmount();
});

test("an initial audio-only configuration is normalized before displaying the selector", async () => {
  const card = mountCard("audio");
  await nextTick();
  expect(card.state.selectedAudioFormat).toBe("en");
  card.unmount();
});

test("user codec filters update the existing settings store", async () => {
  const settings = useSettingStore();
  const card = mountCard();
  card.state.videoFormats.push({ ...format("av1", true), vcodec: "av01.0.08M.08" });
  card.state.audioFormats.push({ ...format("aac"), acodec: "mp4a.40.2" });
  await nextTick();
  card.selectVideoCodec("av1");
  card.selectAudioCodec("aac");
  await nextTick();
  expect(settings.preferredVideoCodec).toBe("av1");
  expect(settings.preferredAudioCodec).toBe("aac");
  expect(card.state.selectedVideoFormat).toBe("av1");
  expect(card.state.selectedAudioFormat).toBe("aac");
  card.selectVideoCodec("all");
  expect(settings.preferredVideoCodec).toBe("all");
  card.unmount();
});

test("a reused card exposes existing selections when switching pending items", async () => {
  const settings = useSettingStore();
  settings.preferredVideoCodec = "h264";
  settings.preferredAudioCodec = "opus";
  const card = mountCard();
  expect(card.videoCodec()).toBe("h264");
  expect(card.audioCodec()).toBe("opus");

  card.state.videoInfo = { ...videoInfo, title: "Other item" };
  card.state.videoFormats = [
    format("video", true),
    { ...format("av1", true), vcodec: "av01.0.08M.08" },
  ];
  card.state.audioFormats = [format("opus"), { ...format("aac"), acodec: "mp4a.40.2" }];
  card.state.selectedVideoFormat = "av1";
  card.state.selectedAudioFormat = "opus+aac";
  await nextTick();
  expect(card.videoCodec()).toBe("all");
  expect(card.audioCodec()).toBe("all");
  expect(card.visibleVideoFormats().map((format) => format.format_id)).toContain("av1");
  expect(card.visibleAudioFormats()).toHaveLength(2);
  expect(card.state.selectedVideoFormat).toBe("av1");
  expect(card.state.selectedAudioFormat).toBe("opus+aac");
  expect(settings.preferredVideoCodec).toBe("h264");
  expect(settings.preferredAudioCodec).toBe("opus");
  card.unmount();
});

test("unsupported saved codecs keep available selectors visible without changing preferences", () => {
  const settings = useSettingStore();
  settings.preferredVideoCodec = "hevc";
  settings.preferredAudioCodec = "aac";
  const card = mountCard();
  expect(card.videoCodec()).toBe("all");
  expect(card.audioCodec()).toBe("all");
  expect(card.visibleVideoFormats()).toHaveLength(1);
  expect(card.visibleAudioFormats()).toHaveLength(2);
  expect(settings.preferredVideoCodec).toBe("hevc");
  expect(settings.preferredAudioCodec).toBe("aac");
  card.unmount();
});
