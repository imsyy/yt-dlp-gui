import { describe, it, expect } from "vitest";
import type { VideoFormat } from "@/types";
import {
  compareVideoFormats,
  detectAudioRole,
  normalizeVideoFormat,
  resolveFileSize,
} from "@/utils/normalizer";

const sampleVideoFormat = (overrides: Partial<VideoFormat>): VideoFormat => ({
  format_id: "137",
  ext: "mp4",
  resolution: "1920x1080",
  height: 1080,
  width: 1920,
  fps: 30,
  vcodec: "avc1.640028",
  acodec: "none",
  filesize: null,
  filesize_approx: null,
  format_note: "1080p",
  tbr: 2500,
  abr: null,
  ...overrides,
});

describe("normalizer utils", () => {
  it("resolveFileSize prefers exact filesize over approx", () => {
    const format = sampleVideoFormat({ filesize: 50_000_000, filesize_approx: 48_000_000 });
    const result = resolveFileSize(format);
    expect(result.size).toBe(50_000_000);
    expect(result.isEstimated).toBe(false);
  });

  it("resolveFileSize uses yt-dlp's approx if exact filesize is null", () => {
    const format = sampleVideoFormat({ filesize: null, filesize_approx: 48_000_000 });
    const result = resolveFileSize(format);
    expect(result.size).toBe(48_000_000);
    expect(result.isEstimated).toBe(true);
  });

  it("resolveFileSize returns 0 when yt-dlp does not provide size (no artificial guessing)", () => {
    const format = sampleVideoFormat({ filesize: null, filesize_approx: null, tbr: 2000 });
    const result = resolveFileSize(format);
    expect(result.size).toBe(0);
    expect(result.isEstimated).toBe(false);
  });

  it("normalizeVideoFormat uses '—' as placeholder when size is missing", () => {
    const format = sampleVideoFormat({ filesize: null, filesize_approx: null });
    const normalized = normalizeVideoFormat(format);
    expect(normalized.filesizeLabel).toBe("—");
  });

  it("compareVideoFormats prioritizes 60fps over 30fps at the same resolution (Fixes #45)", () => {
    const f30 = sampleVideoFormat({ format_id: "f30", height: 1080, fps: 30 });
    const f60 = sampleVideoFormat({ format_id: "f60", height: 1080, fps: 60 });
    const f4k = sampleVideoFormat({ format_id: "f4k", height: 2160, fps: 30 });

    const sorted = [f30, f4k, f60].sort(compareVideoFormats);
    expect(sorted.map((f) => f.format_id)).toEqual(["f4k", "f60", "f30"]);
  });

  it("compareVideoFormats prioritizes higher bitrate when resolution and fps are equal", () => {
    const lowBitrate = sampleVideoFormat({ format_id: "low", height: 1080, fps: 60, vbr: 3000 });
    const highBitrate = sampleVideoFormat({ format_id: "high", height: 1080, fps: 60, vbr: 5000 });

    const sorted = [lowBitrate, highBitrate].sort(compareVideoFormats);
    expect(sorted.map((f) => f.format_id)).toEqual(["high", "low"]);
  });

  it("normalizeVideoFormat produces structured, non-empty fields with exact size", () => {
    const format = sampleVideoFormat({
      format_id: "299",
      height: 1080,
      fps: 60,
      vcodec: "av01.0.08M.08",
      dynamic_range: "HDR",
      filesize: 104_857_600, // 100 MB
    });
    const normalized = normalizeVideoFormat(format);

    expect(normalized.formatId).toBe("299");
    expect(normalized.resolutionLabel).toBe("1080p");
    expect(normalized.fpsLabel).toBe("60fps");
    expect(normalized.codec).toBe("AV1");
    expect(normalized.dynamicRange).toBe("HDR");
    expect(normalized.filesizeLabel).toBe("100.0 MB");
    expect(normalized.isEstimatedSize).toBe(false);
  });

  it("detectAudioRole detects original, dubbed, and descriptive tracks (Fixes #52)", () => {
    expect(detectAudioRole({ format_note: "original (default)" } as VideoFormat).role).toBe("original");
    expect(detectAudioRole({ format_note: "English - dubbed" } as VideoFormat).role).toBe("dubbed");
    expect(detectAudioRole({ format_note: "Descriptive audio" } as VideoFormat).role).toBe("descriptive");
  });
});
