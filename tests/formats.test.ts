import { describe, it, expect } from "vitest";
import type { VideoFormat } from "@/types";
import { compareAudioFormats, getCodecKey, getCodecLabel } from "@/utils/formats";

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

describe("formats utils", () => {
  it("prefers yt-dlp's original-language priority over bitrate", () => {
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

    expect(formats[0].format_id).toBe("251-0");
  });

  it("uses original marker and non-DRC audio as stable fallbacks", () => {
    const formats = [
      audioFormat({ format_id: "dub", format_note: "English - dubbed", abr: 160 }),
      audioFormat({ format_id: "drc", format_note: "Spanish - original, DRC", abr: 140 }),
      audioFormat({ format_id: "original", format_note: "Spanish - original", abr: 128 }),
    ].sort(compareAudioFormats);

    expect(formats.map((format) => format.format_id)).toEqual(["original", "drc", "dub"]);
  });

  it("normalizes common yt-dlp codec identifiers", () => {
    expect(getCodecKey("avc1.640028")).toBe("h264");
    expect(getCodecLabel("av01.0.08M.08")).toBe("AV1");
    expect(getCodecLabel("mp4a.40.2")).toBe("AAC");
  });
});
