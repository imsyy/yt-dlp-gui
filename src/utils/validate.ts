/** 检查字符串是否为有效的 URL */
export const isValidUrl = (str: string): boolean => {
  try {
    const url = new URL(str);
    return url.protocol === "http:" || url.protocol === "https:";
  } catch {
    return false;
  }
};

/** Times are seconds; unknown or live durations do not impose an upper bound. */
export const isValidTimeRange = (
  start: number | null,
  end: number | null,
  duration?: number,
): boolean => {
  if (start == null && end == null) return true;
  const from = start ?? 0;
  if (!Number.isFinite(from) || from < 0) return false;
  if (end != null && (!Number.isFinite(end) || end <= from)) return false;
  if (duration != null && Number.isFinite(duration) && duration > 0) {
    if (from >= duration || (end != null && end > duration)) return false;
  }
  return true;
};
