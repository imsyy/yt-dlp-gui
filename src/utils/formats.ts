import type { VideoFormat } from "@/types";

const audioRoleRank = (format: VideoFormat): number => {
  const description = `${format.format_note || ""} ${format.format || ""}`.toLowerCase();
  if (description.includes("original")) return 3;
  if (description.includes("audio description") || description.includes("descriptive")) return -2;
  if (description.includes("dubbed") || description.includes("translated")) return -1;
  if (description.includes("default")) return 2;
  return 0;
};

/**
 * 保留 yt-dlp 的语言偏好语义，优先原声，再按是否 DRC、码率排序。
 * 旧逻辑只比较码率，会把高码率配音轨误设为默认值。
 */
export const compareAudioFormats = (a: VideoFormat, b: VideoFormat): number => {
  if (a.language_preference != null && b.language_preference != null) {
    const preferenceDifference = b.language_preference - a.language_preference;
    if (preferenceDifference !== 0) return preferenceDifference;
  }

  const roleDifference = audioRoleRank(b) - audioRoleRank(a);
  if (roleDifference !== 0) return roleDifference;

  const aDrc = /\bdrc\b/i.test(a.format_note || "") ? 1 : 0;
  const bDrc = /\bdrc\b/i.test(b.format_note || "") ? 1 : 0;
  if (aDrc !== bDrc) return aDrc - bDrc;

  return (b.abr || b.tbr || 0) - (a.abr || a.tbr || 0);
};
