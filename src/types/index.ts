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
