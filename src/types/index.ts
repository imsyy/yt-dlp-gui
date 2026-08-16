export type ToolSource = "managed" | "system" | "custom";

export interface CliOpenRequest {
  url: string | null;
  cookieFile: string | null;
  downloadDir: string | null;
}

export interface BrowserExtensionImport {
  url: string;
  requestId: string;
  cookieFile: string | null;
  cookieCount: number;
}

export interface ToolStatus {
  installed: boolean;
  version: string;
  path: string;
  source: ToolSource;
  isManaged: boolean;
  canUpdate: boolean;
}

export interface ToolUpdateCheck {
  updateAvailable: boolean;
  currentVersion: string;
  latestVersion: string;
}

export type YtdlpStatus = ToolStatus;
export type DenoStatus = ToolStatus;
export type FfmpegStatus = ToolStatus;

export interface ToolOperationProgress {
  tool: "yt-dlp" | "deno" | "ffmpeg";
  operation: "install" | "update";
  stage: "downloading" | "installing" | "updating" | "complete";
  percent: number | null;
}

export interface DownloadProgress {
  percent: number;
  downloaded: number;
  total: number;
}

export interface VideoFormat {
  format_id: string;
  format?: string;
  ext: string;
  resolution: string;
  height: number | null;
  width: number | null;
  fps: number | null;
  vcodec: string;
  acodec: string;
  filesize: number | null;
  filesize_approx: number | null;
  format_note: string;
  language?: string;
  /** yt-dlp 提取器计算的语言优先级，数值越高越优先。 */
  language_preference?: number | null;
  audio_channels?: number | null;
  dynamic_range?: string | null;
  tbr: number | null;
  abr: number | null;
}

export interface ExtraOptions {
  embedSubs: boolean;
  embedThumbnail: boolean;
  embedMetadata: boolean;
  embedChapters: boolean;
  sponsorblockRemove: boolean;
  extractAudio: boolean;
  audioConvertFormat: string;
  noMerge: boolean;
  recodeFormat: string;
  limitRate: string;
  ffmpegArgs: string;
}

export interface DownloadTaskParams {
  url: string;
  downloadDir: string;
  downloadMode: string;
  videoFormat: string | null;
  audioFormat: string | null;
  cookieFile: string | null;
  cookieBrowser: string | null;
  proxy: string | null;
  outputTemplate: string | null;
  concurrentFragments: number | null;
  noOverwrites: boolean;
  embedSubs: boolean;
  embedThumbnail: boolean;
  embedMetadata: boolean;
  embedChapters: boolean;
  sponsorblockRemove: boolean;
  extractAudio: boolean;
  audioConvertFormat: string | null;
  noMerge: boolean;
  recodeFormat: string | null;
  limitRate: string | null;
  ffmpegArgs: string | null;
  subtitles: string[];
  startTime: number | null;
  endTime: number | null;
  noPlaylist: boolean;
  playlistItems: string | null;
  /** 从开始下载直播流 */
  liveFromStart: boolean;
}

export interface DownloadTask {
  id: string;
  url: string;
  title: string;
  thumbnail: string;
  formatLabel: string;
  status:
    | "queued"
    | "downloading"
    | "postprocessing"
    | "paused"
    | "completed"
    | "error"
    | "cancelled";
  percent: number;
  speed: string;
  eta: string;
  downloaded: string;
  total: string;
  logs: string[];
  error?: string;
  outputFile?: string;
  createdAt: number;
  params: DownloadTaskParams;
}

export interface FetchedVideoData {
  url: string;
  videoInfo: VideoInfo;
  videoFormats: VideoFormat[];
  audioFormats: VideoFormat[];
  isPlaylist: boolean;
  playlistEntries: PlaylistEntry[];
}

export interface PendingItem extends FetchedVideoData {
  id: string;
  createdAt: number;
  selectedPlaylistItems: number[];
  downloadMode: "default" | "video" | "audio";
  selectedVideoFormat: string;
  selectedAudioFormat: string;
  startTime: number | null;
  endTime: number | null;
  embedSubs: boolean;
  embedThumbnail: boolean;
  embedMetadata: boolean;
  embedChapters: boolean;
  sponsorblockRemove: boolean;
  extractAudio: boolean;
  audioConvertFormat: string;
  noMerge: boolean;
  recodeFormat: string;
  limitRate: string;
  ffmpegArgs: string;
  selectedSubtitles: string[];
  /** 是否从开始下载直播流（--live-from-start） */
  liveFromStart: boolean;
}

export interface PlaylistEntry {
  id: string;
  title: string;
  duration: number | null;
  url: string;
  thumbnail?: string;
  formats?: VideoFormat[];
  subtitles?: Record<string, { ext: string; url: string; name?: string }[]>;
  automatic_captions?: Record<string, { ext: string; url: string; name?: string }[]>;
}

export interface ThumbnailInfo {
  url: string;
  height?: number;
  width?: number;
  resolution?: string;
  id?: string;
}

export interface VideoInfo {
  title: string;
  thumbnail: string;
  thumbnails?: ThumbnailInfo[];
  duration: number;
  uploader: string;
  view_count: number;
  upload_date: string;
  description: string;
  formats: VideoFormat[];
  subtitles: Record<string, { ext: string; url: string; name?: string }[]>;
  automatic_captions: Record<string, { ext: string; url: string; name?: string }[]>;
  /** yt-dlp 直播状态: "not_live" | "is_live" | "is_upcoming" | "was_live" | "post_live" */
  live_status?: string;
  /** 是否为直播流（yt-dlp 布尔字段） */
  is_live?: boolean;
  /** Playlist fields — present when the URL is a playlist */
  _type?: string;
  entries?: PlaylistEntry[];
  playlist_count?: number;
}

export interface Chapter {
  title: string;
  start_time: number;
  end_time: number;
}

export interface ChaptersInfo {
  title: string;
  duration: number | null;
  chapters: Chapter[];
}

export interface VideoComment {
  id: string;
  parent: string;
  author: string;
  author_id: string;
  text: string;
  timestamp: number;
  like_count: number;
  is_favorited: boolean;
  author_is_uploader: boolean;
}

export interface CommentsInfo {
  title: string;
  comment_count: number | null;
  comments: VideoComment[];
}

export interface LiveChatMessage {
  idx: number;
  time: string;
  timestamp_usec: number;
  author: string;
  channel_id: string;
  message: string;
  msg_type: string;
  amount: string;
}

export interface SubtitleTrack {
  ext: string;
  url: string;
  name?: string;
}

export interface SubtitleInfo {
  title: string;
  subtitles: Record<string, SubtitleTrack[]>;
  automatic_captions: Record<string, SubtitleTrack[]>;
}
