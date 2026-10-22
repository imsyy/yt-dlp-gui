import { expect, test, vi } from "vitest";
import { createRenderer, getCurrentInstance, h, ssrContextKey } from "vue";
import DownloadOptionsCard from "@/components/home/DownloadOptionsCard.vue";
import type { VideoFormat, VideoInfo } from "@/types";
import type { Component, ComponentInternalInstance } from "vue";

vi.mock("vue-i18n", () => ({ useI18n: () => ({ t: (key: string) => key }) }));

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
        videoInfo,
        videoFormats: [format("video", true)],
        audioFormats: [format("en"), format("es")],
        "onUpdate:selectedAudioFormat": (value: string) => {
          state.selectedAudioFormat = value;
        },
      }),
  });
  // Vitest transforms SFCs for SSR, whose setup registers this module in the context.
  app.provide(ssrContextKey, { modules: new Set() });
  app.mount({});
  return { state, multipleAudio: () => componentState.multipleAudio, unmount: () => app.unmount() };
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
