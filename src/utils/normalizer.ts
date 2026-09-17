import type { NormalizedAudioFormat, NormalizedVideoFormat, VideoFormat } from "@/types";
import { getCodecKey, getCodecLabel } from "./formats.ts";

/** 格式化文件大小为可读字符串（B / KB / MB / GB） */
export const formatFileSize = (bytes: number): string => {
  if (!bytes || bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let size = bytes;
  let i = 0;
  while (size >= 1024 && i < units.length - 1) {
    size /= 1024;
    i++;
  }
  return `${size.toFixed(1)} ${units[i]}`;
};

/**
 * 提取 yt-dlp 提供的文件大小。
 * 严格基于 yt-dlp 数据源：确切大小 (filesize) > 官方预估 (filesize_approx)。
 * 若源站未提供，返回 0，显示端统一呈现标准占位符 "—"，不进行人工主观猜测推算。
 */
export const resolveFileSize = (format: VideoFormat): { size: number; isEstimated: boolean } => {
  if (typeof format.filesize === "number" && format.filesize > 0) {
    return { size: format.filesize, isEstimated: false };
  }
  if (typeof format.filesize_approx === "number" && format.filesize_approx > 0) {
    return { size: format.filesize_approx, isEstimated: true };
  }
  return { size: 0, isEstimated: false };
};

export const estimateFileSize = (
  format: VideoFormat,
  _duration?: number | null,
): { size: number; isEstimated: boolean } => resolveFileSize(format);

/**
 * 格式化分辨率描述
 */
export const formatResolutionLabel = (height?: number | null, _width?: number | null): string => {
  if (!height) return "未知清晰度";
  if (height >= 4320) return `8K (${height}p)`;
  if (height >= 2160) return `4K (${height}p)`;
  if (height >= 1440) return `2K (${height}p)`;
  if (height >= 1080) return `1080p`;
  if (height >= 720) return `720p`;
  if (height >= 480) return `480p`;
  if (height >= 360) return `360p`;
  return `${height}p`;
};

/**
 * 格式化帧率描述
 */
export const formatFpsLabel = (fps?: number | null): string => {
  if (!fps || fps <= 0) return "";
  return `${Math.round(fps)}fps`;
};

/**
 * 常见音轨语言代号转本地化展示
 */
const LANGUAGE_NAMES: Record<string, string> = {
  zh: "中文",
  "zh-hans": "中文(简体)",
  "zh-hant": "中文(繁体)",
  "zh-cn": "中文(大陆)",
  "zh-tw": "中文(台湾)",
  "zh-hk": "中文(香港)",
  en: "英语",
  ja: "日语",
  ko: "韩语",
  es: "西班牙语",
  fr: "法语",
  de: "德语",
  ru: "俄语",
  pt: "葡萄牙语",
  it: "意大利语",
  vi: "越南语",
  th: "泰语",
  ar: "阿拉伯语",
  hi: "印地语",
};

export const formatLanguageLabel = (languageCode?: string): string => {
  if (!languageCode) return "";
  const key = languageCode.toLowerCase().trim();
  return LANGUAGE_NAMES[key] || LANGUAGE_NAMES[key.split("-")[0]] || languageCode.toUpperCase();
};

/**
 * 提取音频轨道角色
 */
export const detectAudioRole = (
  format: VideoFormat,
): { role: "original" | "dubbed" | "descriptive" | "default" | "unknown"; label: string } => {
  const desc = `${format.format_note || ""} ${format.format || ""}`.toLowerCase();
  if (desc.includes("original")) {
    return { role: "original", label: "原声" };
  }
  if (desc.includes("dubbed") || desc.includes("translated")) {
    return { role: "dubbed", label: "配音" };
  }
  if (desc.includes("audio description") || desc.includes("descriptive")) {
    return { role: "descriptive", label: "旁白" };
  }
  if (desc.includes("default")) {
    return { role: "default", label: "默认" };
  }
  return { role: "unknown", label: "" };
};

/**
 * 规范化单个视频格式对象
 */
export const normalizeVideoFormat = (
  format: VideoFormat,
  duration?: number | null,
): NormalizedVideoFormat => {
  const { size, isEstimated } = estimateFileSize(format, duration);
  const codecKey = getCodecKey(format.vcodec);
  const codec = getCodecLabel(format.vcodec);
  const height = format.height || 0;
  const width = format.width || 0;
  const fps = format.fps ? Math.round(format.fps) : 0;
  const fpsLabel = formatFpsLabel(fps);
  const resolutionLabel = formatResolutionLabel(height, width);
  const dynamicRange = format.dynamic_range || "";
  const container = (format.ext || "mp4").toUpperCase();
  const bitrate = format.vbr || format.tbr || 0;

  const filesizeLabel =
    size > 0 ? (isEstimated ? `~${formatFileSize(size)}` : formatFileSize(size)) : "—";

  return {
    formatId: format.format_id,
    container,
    resolutionLabel,
    height,
    width,
    fps,
    fpsLabel,
    codec,
    codecKey,
    dynamicRange,
    filesize: size,
    filesizeLabel,
    isEstimatedSize: isEstimated,
    bitrate,
    raw: format,
  };
};

/**
 * 规范化单个音频格式对象
 */
export const normalizeAudioFormat = (
  format: VideoFormat,
  duration?: number | null,
): NormalizedAudioFormat => {
  const { size, isEstimated } = estimateFileSize(format, duration);
  const codecKey = getCodecKey(format.acodec);
  const codec = getCodecLabel(format.acodec);
  const container = (format.ext || "m4a").toUpperCase();
  const channels = format.audio_channels || 2;
  const channelsLabel = channels === 1 ? "单声道" : channels === 2 ? "立体声" : `${channels}声道`;
  const language = format.language || "";
  const languageLabel = formatLanguageLabel(language);
  const { role, label: roleLabel } = detectAudioRole(format);
  const bitrate = format.abr || format.tbr || 0;
  const bitrateLabel = bitrate > 0 ? `${Math.round(bitrate)} kbps` : "";
  const filesizeLabel =
    size > 0 ? (isEstimated ? `~${formatFileSize(size)}` : formatFileSize(size)) : "—";

  return {
    formatId: format.format_id,
    container,
    codec,
    codecKey,
    channels,
    channelsLabel,
    language,
    languageLabel,
    role,
    roleLabel,
    bitrate,
    bitrateLabel,
    filesize: size,
    filesizeLabel,
    isEstimatedSize: isEstimated,
    raw: format,
  };
};

/**
 * 规范化视频流对比排序
 * 规则：
 * 1. 分辨率高度 (height) 降序
 * 2. 帧率 (fps) 降序 (60fps > 30fps，解决 Issue #45)
 * 3. 码率降序 (高质量优先)
 * 4. 文件大小降序
 */
export const compareVideoFormats = (a: VideoFormat, b: VideoFormat): number => {
  const hDiff = (b.height || 0) - (a.height || 0);
  if (hDiff !== 0) return hDiff;

  const fpsDiff = (b.fps || 0) - (a.fps || 0);
  if (fpsDiff !== 0) return fpsDiff;

  const brA = a.vbr || a.tbr || 0;
  const brB = b.vbr || b.tbr || 0;
  if (brB !== brA) return brB - brA;

  const sizeA = a.filesize || a.filesize_approx || 0;
  const sizeB = b.filesize || b.filesize_approx || 0;
  return sizeB - sizeA;
};
