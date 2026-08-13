const YOUTUBE_PLAYLIST_CONTEXT_PARAMS = ["list", "index", "start_radio", "playnext"];

/**
 * 浏览器扩展发送 YouTube 播放页时只解析当前视频，避免 `list` 参数让 yt-dlp
 * 展开整个播放列表。真正的 `/playlist` URL 以及其他站点保持不变。
 */
export const normalizeDeepLinkVideoUrl = (value: string): string => {
  try {
    const url = new URL(value);
    const hostname = url.hostname.toLowerCase();
    const isYoutubeHost =
      hostname === "youtu.be" || hostname === "youtube.com" || hostname.endsWith(".youtube.com");
    const isSingleVideo =
      (hostname === "youtu.be" && url.pathname.length > 1) ||
      (url.pathname === "/watch" && url.searchParams.has("v"));

    if (isYoutubeHost && isSingleVideo) {
      for (const parameter of YOUTUBE_PLAYLIST_CONTEXT_PARAMS) {
        url.searchParams.delete(parameter);
      }
    }
    return url.toString();
  } catch {
    return value;
  }
};
