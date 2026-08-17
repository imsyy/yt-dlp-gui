import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "vue-i18n";
import { composeOutputTemplate } from "@/utils/output-template";
import { useDownloadStore } from "@/stores/download";
import { useSettingStore } from "@/stores/setting";
import { useStatusStore } from "@/stores/status";
import { useVideoStore } from "@/stores/video";
import type { DownloadTask, FfmpegStatus, PendingItem } from "@/types";

export type DownloadLaunchResult =
  | "started"
  | "queued"
  | "missing-directory"
  | "missing-ffmpeg"
  | "failed";

const timeToSeconds = (timestamp: number): number => {
  const date = new Date(timestamp);
  return date.getHours() * 3600 + date.getMinutes() * 60 + date.getSeconds();
};

const formatTime = (seconds: number): string => {
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const secs = seconds % 60;
  const pad = (value: number) => String(value).padStart(2, "0");
  return hours > 0
    ? `${pad(hours)}:${pad(minutes)}:${pad(secs)}`
    : `${pad(minutes)}:${pad(secs)}`;
};

export const useDownloadLauncher = () => {
  const { t } = useI18n();
  const settingStore = useSettingStore();
  const downloadStore = useDownloadStore();
  const videoStore = useVideoStore();
  const statusStore = useStatusStore();

  const buildFormatLabel = (item: PendingItem): string => {
    const parts: string[] = [];
    if (item.downloadMode === "audio") {
      parts.push(t("detail.audioOnly"));
      const audio = item.audioFormats.find(
        (format) => format.format_id === item.selectedAudioFormat,
      );
      if (audio) parts.push(audio.format_note || audio.ext);
    } else {
      const video = item.videoFormats.find(
        (format) => format.format_id === item.selectedVideoFormat,
      );
      if (video?.height) parts.push(`${video.height}p`);
      if (video?.fps) parts.push(`${video.fps}fps`);
      if (item.downloadMode === "video") parts.push(t("detail.videoOnly"));
    }

    if (item.startTime != null || item.endTime != null) {
      const start =
        item.startTime != null ? formatTime(timeToSeconds(item.startTime)) : "00:00";
      const end =
        item.endTime != null ? formatTime(timeToSeconds(item.endTime)) : t("detail.end");
      parts.push(`✂${start}-${end}`);
    }
    return parts.join(" ") || t("detail.defaultQuality");
  };

  const createPreparingTask = (url: string): string => {
    const taskId = `prepare_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`;
    downloadStore.addTask({
      id: taskId,
      url,
      title: url,
      thumbnail: "",
      formatLabel: t("downloads.status.preparing"),
      status: "preparing",
      percent: 0,
      speed: "",
      eta: "",
      downloaded: "",
      total: "",
      logs: [],
      createdAt: Date.now(),
      params: {
        url,
        downloadDir: settingStore.downloadDir,
        downloadMode: settingStore.quickDownloadMode,
        videoFormat: null,
        audioFormat: null,
        cookieFile: null,
        cookieBrowser: null,
        proxy: settingStore.proxy || null,
        outputTemplate: null,
        concurrentFragments: null,
        noOverwrites: settingStore.noOverwrites,
        embedSubs: false,
        embedThumbnail: false,
        embedMetadata: false,
        embedChapters: false,
        sponsorblockRemove: false,
        extractAudio: false,
        audioConvertFormat: null,
        noMerge: false,
        recodeFormat: null,
        limitRate: null,
        ffmpegArgs: null,
        subtitles: [],
        startTime: null,
        endTime: null,
        noPlaylist: false,
        playlistItems: null,
        liveFromStart: false,
      },
    });
    return taskId;
  };

  const markPreparationError = (taskId: string) => {
    const task = downloadStore.tasks.find((item) => item.id === taskId);
    if (!task || task.status !== "preparing") return;
    task.status = "error";
    task.error = t("home.quickPrepareFailed");
  };

  const launchDownload = async (
    item: PendingItem,
    preparingTaskId?: string,
  ): Promise<DownloadLaunchResult> => {
    if (!settingStore.downloadDir) {
      window.$message.warning(t("detail.setDownloadDirFirst"));
      return "missing-directory";
    }

    const requiresFfmpegMerge =
      item.downloadMode === "default" &&
      Boolean(item.selectedVideoFormat) &&
      Boolean(item.selectedAudioFormat) &&
      !item.noMerge;
    if (requiresFfmpegMerge) {
      try {
        const status = await invoke<FfmpegStatus>("get_ffmpeg_status");
        if (!status.installed) {
          statusStore.showFfmpegSetupModal = true;
          if (preparingTaskId) markPreparationError(preparingTaskId);
          return "missing-ffmpeg";
        }
      } catch {
        statusStore.showFfmpegSetupModal = true;
        if (preparingTaskId) markPreparationError(preparingTaskId);
        return "missing-ffmpeg";
      }
    }

    const taskId = `dl_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`;
    const { cookieFile, cookieBrowser } = await videoStore.getCookieArgs();
    const params = {
      url: item.url,
      downloadDir: settingStore.downloadDir,
      downloadMode: item.downloadMode,
      videoFormat: item.selectedVideoFormat || null,
      audioFormat: item.selectedAudioFormat || null,
      cookieFile,
      cookieBrowser,
      proxy: settingStore.proxy || null,
      outputTemplate: composeOutputTemplate(
        settingStore.outputTemplate,
        settingStore.filenamePrefix,
        settingStore.filenameSuffix,
      ),
      concurrentFragments: settingStore.concurrentFragments || null,
      noOverwrites: settingStore.noOverwrites,
      embedSubs: item.embedSubs,
      embedThumbnail: item.embedThumbnail,
      embedMetadata: item.embedMetadata,
      embedChapters: item.embedChapters,
      sponsorblockRemove: item.sponsorblockRemove,
      extractAudio: item.extractAudio,
      audioConvertFormat: item.audioConvertFormat || null,
      noMerge: item.noMerge,
      recodeFormat: item.recodeFormat || null,
      limitRate: item.limitRate || null,
      ffmpegArgs: item.ffmpegArgs || null,
      subtitles: item.selectedSubtitles,
      startTime: item.startTime != null ? timeToSeconds(item.startTime) : null,
      endTime: item.endTime != null ? timeToSeconds(item.endTime) : null,
      liveFromStart: item.liveFromStart,
      noPlaylist: item.isPlaylist && item.selectedPlaylistItems.length === 1,
      playlistItems:
        item.isPlaylist && item.selectedPlaylistItems.length > 0
          ? item.selectedPlaylistItems
              .slice()
              .sort((a, b) => a - b)
              .join(",")
          : null,
    };
    const shouldQueue = !downloadStore.canStartNow();

    const task: DownloadTask = {
      id: taskId,
      url: item.url,
      title: item.videoInfo.title || t("detail.unknownVideo"),
      thumbnail: item.videoInfo.thumbnail || "",
      formatLabel: buildFormatLabel(item),
      status: shouldQueue ? "queued" : "downloading",
      percent: 0,
      speed: "",
      eta: "",
      downloaded: "",
      total: "",
      logs: [],
      createdAt: Date.now(),
      params,
    };

    if (preparingTaskId) {
      const preparingTask = downloadStore.tasks.find((candidate) => candidate.id === preparingTaskId);
      if (!preparingTask || preparingTask.status !== "preparing") return "failed";
      Object.assign(preparingTask, task);
    } else {
      downloadStore.addTask(task);
    }

    if (shouldQueue) return "queued";

    try {
      await invoke("start_download", { params: { id: taskId, ...params } });
      return "started";
    } catch (error: unknown) {
      window.$message.error(
        error instanceof Error ? error.message : String(error) || t("detail.startDownloadFailed"),
      );
      if (preparingTaskId) {
        const failedTask = downloadStore.tasks.find((candidate) => candidate.id === taskId);
        if (failedTask) {
          failedTask.status = "error";
          failedTask.error =
            error instanceof Error ? error.message : String(error) || t("detail.startDownloadFailed");
        }
      } else {
        downloadStore.removeTask(taskId);
      }
      return "failed";
    }
  };

  return { createPreparingTask, markPreparationError, launchDownload };
};
