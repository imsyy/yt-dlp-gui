import type { DownloadTaskParams } from "@/types";

/**
 * 补齐下载任务参数默认值，防止字段缺失导致后端反序列化报错
 *
 * 老版本 IndexedDB 里存的参数可能缺字段，每次启动排队任务/重试前都会经过这里归一。
 *
 * @param params 原始或部分下载参数
 * @returns 补齐默认值后的完整参数对象
 */
export const normalizeTaskParams = (params?: Partial<DownloadTaskParams>): DownloadTaskParams => ({
  url: params?.url || "",
  downloadDir: params?.downloadDir || "",
  downloadMode: params?.downloadMode || "default",
  videoFormat: params?.videoFormat ?? null,
  audioFormat: params?.audioFormat ?? null,
  cookieFile: params?.cookieFile ?? null,
  cookieBrowser: params?.cookieBrowser ?? null,
  proxy: params?.proxy ?? null,
  outputTemplate: params?.outputTemplate ?? null,
  concurrentFragments: params?.concurrentFragments ?? null,
  noOverwrites: Boolean(params?.noOverwrites),
  embedSubs: Boolean(params?.embedSubs),
  embedThumbnail: Boolean(params?.embedThumbnail),
  writeThumbnail: Boolean(params?.writeThumbnail),
  writeDescription: Boolean(params?.writeDescription),
  embedMetadata: Boolean(params?.embedMetadata),
  embedChapters: Boolean(params?.embedChapters),
  sponsorblockRemove: Boolean(params?.sponsorblockRemove),
  extractAudio: Boolean(params?.extractAudio),
  audioConvertFormat: params?.audioConvertFormat ?? null,
  noMerge: Boolean(params?.noMerge),
  recodeFormat: params?.recodeFormat ?? null,
  remuxFormat: params?.remuxFormat ?? null,
  limitRate: params?.limitRate ?? null,
  ffmpegArgs: params?.ffmpegArgs ?? null,
  customArgs: params?.customArgs ?? null,
  subtitles: Array.isArray(params?.subtitles) ? params.subtitles : [],
  startTime: params?.startTime ?? null,
  endTime: params?.endTime ?? null,
  noPlaylist: Boolean(params?.noPlaylist),
  playlistItems: params?.playlistItems ?? null,
  liveFromStart: Boolean(params?.liveFromStart),
});
