export interface YtdlpStatus {
  installed: boolean;
  version: string;
  path: string;
}

export interface DenoStatus {
  installed: boolean;
  version: string;
  path: string;
}

export interface DownloadProgress {
  percent: number;
  downloaded: number;
  total: number;
}

export interface VideoFormat {
  format_id: string;
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
  tbr: number | null;
  abr: number | null;
}

export interface ExtraOptions {
  embedSubs: boolean;
  embedThumbnail: boolean;
  embedMetadata: boolean;
  noMerge: boolean;
  recodeFormat: string;
  limitRate: string;
}

export interface DownloadTaskParams {
  url: string;
  downloadDir: string;
  downloadMode: string;
  videoFormat: string | null;
  audioFormat: string | null;
  cookieFile: string | null;
  embedSubs: boolean;
  embedThumbnail: boolean;
  embedMetadata: boolean;
  noMerge: boolean;
  recodeFormat: string | null;
  limitRate: string | null;
  subtitles: string[];
  startTime: number | null;
  endTime: number | null;
  noPlaylist: boolean;
  playlistItems: string | null;
}

export interface DownloadTask {
  id: string;
  url: string;
  title: string;
  thumbnail: string;
  formatLabel: string;
  status: "downloading" | "paused" | "completed" | "error" | "cancelled";
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

export interface PlaylistEntry {
  id: string;
  title: string;
  duration: number | null;
  url: string;
  thumbnail?: string;
  formats?: VideoFormat[];
}

export interface VideoInfo {
  title: string;
  thumbnail: string;
  duration: number;
  uploader: string;
  view_count: number;
  upload_date: string;
  description: string;
  formats: VideoFormat[];
  subtitles: Record<string, { ext: string; url: string; name?: string }[]>;
  automatic_captions: Record<
    string,
    { ext: string; url: string; name?: string }[]
  >;
  /** Playlist fields — present when the URL is a playlist */
  _type?: string;
  entries?: PlaylistEntry[];
  playlist_count?: number;
}
