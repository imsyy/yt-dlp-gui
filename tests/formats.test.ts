import assert from "node:assert/strict";
import test from "node:test";
import type { VideoFormat } from "../src/types/index.ts";
import { compareAudioFormats, getCodecKey, getCodecLabel } from "../src/utils/formats.ts";

const audioFormat = (overrides: Partial<VideoFormat>): VideoFormat => ({
  format_id: "251",
  ext: "webm",
  resolution: "audio only",
  height: null,
  width: null,
  fps: null,
  vcodec: "none",
  acodec: "opus",
  filesize: null,
  filesize_approx: null,
  format_note: "",
  tbr: null,
  abr: 128,
  ...overrides,
});

test("prefers yt-dlp's original-language priority over bitrate", () => {
  const formats = [
    audioFormat({
      format_id: "251-1",
      language: "en",
      language_preference: -2,
      format_note: "English - dubbed-auto",
      abr: 160,
    }),
    audioFormat({
      format_id: "251-0",
      language: "es",
      language_preference: -1,
      format_note: "Spanish - original (default)",
      abr: 128,
    }),
  ].sort(compareAudioFormats);

  assert.equal(formats[0].format_id, "251-0");
});

test("uses original marker and non-DRC audio as stable fallbacks", () => {
  const formats = [
    audioFormat({ format_id: "dub", format_note: "English - dubbed", abr: 160 }),
    audioFormat({ format_id: "drc", format_note: "Spanish - original, DRC", abr: 140 }),
    audioFormat({ format_id: "original", format_note: "Spanish - original", abr: 128 }),
  ].sort(compareAudioFormats);

  assert.deepEqual(
    formats.map((format) => format.format_id),
    ["original", "drc", "dub"],
  );
});

test("normalizes common yt-dlp codec identifiers", () => {
  assert.equal(getCodecKey("avc1.640028"), "h264");
  assert.equal(getCodecLabel("av01.0.08M.08"), "AV1");
  assert.equal(getCodecLabel("mp4a.40.2"), "AAC");
});
