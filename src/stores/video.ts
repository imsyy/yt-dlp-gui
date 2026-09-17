import { defineStore } from "pinia";
import { invoke } from "@tauri-apps/api/core";
import { showErrorDialog } from "@/utils/format";
import { filterPlaylistEntries } from "@/utils/playlist";
import { compareAudioFormats } from "@/utils/formats";
import { compareVideoFormats } from "@/utils/normalizer";
import { useSettingStore } from "@/stores/setting";
import { useStatusStore } from "@/stores/status";
import i18n from "@/locales";
import type { VideoInfo, VideoFormat, PlaylistEntry, DenoStatus, FetchedVideoData } from "@/types";

type SubtitleMap = NonNullable<PlaylistEntry["subtitles"]>;

/** 聚合 playlist 各 entry 的字幕到一个并集；同语言取首个出现的 entry 的 tracks */
const aggregateSubtitleMap = (
  entries: PlaylistEntry[],
  field: "subtitles" | "automatic_captions",
): SubtitleMap => {
  const merged: SubtitleMap = {};
  for (const entry of entries) {
    const map = entry[field];
    if (!map) continue;
    for (const [lang, tracks] of Object.entries(map)) {
      if (!merged[lang] && tracks?.length) merged[lang] = tracks;
    }
  }
  return merged;
};

/**
 * 视频元数据获取 Store
 *
 * 负责调用 yt-dlp 解析远程 URL 媒体信息（单视频/播放列表、可用音视频格式、字幕等）。
 */
export const useVideoStore = defineStore("video", () => {
  const fetching = ref(false);

  /**
   * 根据当前全局设置解析并获取有效的 Cookie 传入参数
   *
   * @returns 包含 cookieFile 或 cookieBrowser 的参数对象
   */
  const getCookieArgs = async (): Promise<{
    cookieFile: string | null;
    cookieBrowser: string | null;
  }> => {
    const settingStore = useSettingStore();
    const { cookieMode, cookieText, cookieFile, cookieBrowser } = settingStore;
    if (cookieMode === "text" && cookieText.trim()) {
      const path = await invoke<string>("save_cookie_text", { text: cookieText });
      return { cookieFile: path, cookieBrowser: null };
    }
    if (cookieMode === "file" && cookieFile) {
      return { cookieFile, cookieBrowser: null };
    }
    if (cookieMode === "browser" && cookieBrowser) {
      return { cookieFile: null, cookieBrowser };
    }
    return { cookieFile: null, cookieBrowser: null };
  };

  /**
   * 解析指定 URL 的视频或播放列表元数据
   *
   * @param targetUrl 待解析的视频或播放列表地址
   * @param options 可选配置（如 silent 静默模式，不弹出错误通知框）
   * @returns 解析成功返回标准化视频数据对象，失败则返回 null
   */
  const fetchVideoInfo = async (
    targetUrl: string,
    options: { silent?: boolean } = {},
  ): Promise<FetchedVideoData | null> => {
    const settingStore = useSettingStore();
    fetching.value = true;
    try {
      const { cookieFile, cookieBrowser } = await getCookieArgs();
      const info = await invoke<VideoInfo>("fetch_video_info", {
        url: targetUrl,
        cookieFile,
        cookieBrowser,
        proxy: settingStore.proxy || null,
      });
      let videoInfo: VideoInfo;
      let isPlaylist = false;
      let playlistEntries: PlaylistEntry[] = [];

      const entries = filterPlaylistEntries(info.entries || []);

      if (info._type === "playlist" && entries.length) {
        isPlaylist = true;
        playlistEntries = entries.map((e, i) => ({
          id: e.id || String(i + 1),
          title: e.title || `第 ${i + 1} P`,
          duration: e.duration ?? null,
          url: e.url || "",
        }));
        const firstEntry = entries[0];
        const formats: VideoFormat[] = firstEntry?.formats || info.formats || [];
        // 合集字幕：yt-dlp -J 对 playlist 不会在 root 暴露 subtitles，
        // 必须从各 entry 聚合。同语言的 tracks 取首个出现该语言的 entry。
        videoInfo = {
          ...info,
          title: info.title || firstEntry?.title || "",
          thumbnail: info.thumbnail || firstEntry?.thumbnail || "",
          duration: info.duration || firstEntry?.duration || 0,
          formats,
          subtitles: aggregateSubtitleMap(entries, "subtitles"),
          automatic_captions: aggregateSubtitleMap(entries, "automatic_captions"),
        };
      } else {
        videoInfo = info;
      }

      const formats: VideoFormat[] = videoInfo.formats || [];

      const videoOnlyFormats = formats
        .filter((f) => f.vcodec && f.vcodec !== "none" && (!f.acodec || f.acodec === "none"))
        .sort(compareVideoFormats);
      const combinedFormats = formats
        .filter((f) => f.vcodec && f.vcodec !== "none" && f.acodec && f.acodec !== "none")
        .sort(compareVideoFormats);
      // 部分站点只提供已封装音视频的单文件格式；没有纯视频流时不能把这些格式全部过滤掉。
      const videoFormats = videoOnlyFormats.length ? videoOnlyFormats : combinedFormats;
      const audioFormats = formats
        .filter((f) => f.acodec && f.acodec !== "none" && (!f.vcodec || f.vcodec === "none"))
        .sort(compareAudioFormats);

      // YouTube URL 且 Deno 未安装时提示
      if (/youtube\.com|youtu\.be/i.test(targetUrl)) {
        try {
          const denoStatus = await invoke<DenoStatus>("get_deno_status");
          if (!denoStatus.installed) {
            const statusStore = useStatusStore();
            statusStore.showDenoSetupModal = true;
          }
        } catch {
          // ignore
        }
      }

      return {
        url: targetUrl,
        videoInfo,
        videoFormats,
        audioFormats,
        isPlaylist,
        playlistEntries,
      };
    } catch (e: unknown) {
      const raw = e instanceof Error ? e.message : String(e) || "获取视频信息失败";
      if (/err_ytdlp_not_installed/.test(raw)) {
        const statusStore = useStatusStore();
        statusStore.showYtdlpSetupModal = true;
      } else if (/Could not copy.*cookie database/i.test(raw)) {
        showErrorDialog(raw);
      } else if (/sign in|cookies/i.test(raw)) {
        window.$message.warning(i18n.global.t("cookie.verificationDesc"));
        const statusStore = useStatusStore();
        statusStore.showCookieModal = true;
      } else if (!options.silent) {
        showErrorDialog(raw);
      }
      return null;
    } finally {
      fetching.value = false;
    }
  };

  return {
    fetching,
    fetchVideoInfo,
    getCookieArgs,
  };
});
