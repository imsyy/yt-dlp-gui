<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { formatFileSize } from "@/utils/format";
import { isValidUrl } from "@/utils/validate";
import { useSettingStore } from "@/stores/setting";
import { useDownloadStore } from "@/stores/download";
import type { VideoInfo, VideoFormat, PlaylistEntry } from "@/types";
import VideoInfoCard from "@/components/home/VideoInfoCard.vue";
import DownloadOptionsCard from "@/components/home/DownloadOptionsCard.vue";
import ExtraOptionsCard from "@/components/home/ExtraOptionsCard.vue";
import SubtitleCard from "@/components/home/SubtitleCard.vue";
import DownloadDirCard from "@/components/DownloadDirCard.vue";
import DownloadBar from "@/components/home/DownloadBar.vue";

const router = useRouter();
const settingStore = useSettingStore();
const downloadStore = useDownloadStore();

// ========== 状态 ==========
const url = ref("");
const fetching = ref(false);
const detailMode = ref(false);
const videoInfo = ref<VideoInfo | null>(null);
const videoFormats = ref<VideoFormat[]>([]);
const audioFormats = ref<VideoFormat[]>([]);

// 下载模式
const downloadMode = ref<"default" | "video" | "audio">("default");

// 格式选择
const selectedVideoFormat = ref("");
const selectedAudioFormat = ref("");

// 额外选项
const startTime = ref<number | null>(null);
const endTime = ref<number | null>(null);
const embedSubs = ref(false);
const embedThumbnail = ref(false);
const embedMetadata = ref(false);
const noMerge = ref(false);
const recodeFormat = ref("");
const limitRate = ref("");
const selectedSubtitles = ref<string[]>([]);

// 播放列表
const isPlaylist = ref(false);
const playlistEntries = ref<PlaylistEntry[]>([]);
const selectedPlaylistItems = ref<number[]>([]);

// 下载目录卡片 ref
const dirCardRef = ref<HTMLElement | null>(null);

// 提示文本
const tips = [
  "遇到下载问题？请前往设置中填写 Cookie 或更新 yt-dlp 版本",
  "支持 YouTube、Bilibili、Twitter 等上千个网站",
  "可选仅视频或仅音频模式，按需下载",
  "下载前请确保已设置下载目录",
  "YouTube 视频建议安装 Deno 运行时以获取完整格式列表",
];

// 当前提示索引
const currentTipIndex = ref(0);
let tipTimer: ReturnType<typeof setInterval> | null = null;

onMounted(() => {
  tipTimer = setInterval(() => {
    currentTipIndex.value = (currentTipIndex.value + 1) % tips.length;
  }, 4000);
});

onUnmounted(() => {
  if (tipTimer) clearInterval(tipTimer);
});

// ========== 计算 ==========
const estimatedSize = computed(() => {
  let total = 0;
  if (downloadMode.value !== "audio") {
    const vf = videoFormats.value.find(
      (f) => f.format_id === selectedVideoFormat.value,
    );
    if (vf) total += vf.filesize || vf.filesize_approx || 0;
  }
  if (downloadMode.value !== "video") {
    const af = audioFormats.value.find(
      (f) => f.format_id === selectedAudioFormat.value,
    );
    if (af) total += af.filesize || af.filesize_approx || 0;
  }
  return total;
});

const estimatedSizeText = computed(() => {
  if (!estimatedSize.value) return "未知";
  return formatFileSize(estimatedSize.value);
});

// ========== 方法 ==========

/** 获取当前有效的 Cookie 文件路径 */
const getCookieFile = async (): Promise<string | null> => {
  const { cookieMode, cookieText, cookieFile } = settingStore;
  if (cookieMode === "text" && cookieText.trim()) {
    return await invoke<string>("save_cookie_text", { text: cookieText });
  }
  if (cookieMode === "file" && cookieFile) {
    return cookieFile;
  }
  return null;
};

/** 解析视频链接，获取视频信息与可用格式 */
const handleSearch = async () => {
  if (!url.value.trim()) return;
  if (!isValidUrl(url.value.trim())) {
    window.$message.warning("请输入有效的网址");
    return;
  }
  fetching.value = true;
  try {
    const cookieFile = await getCookieFile();
    const info = await invoke<VideoInfo>("fetch_video_info", {
      url: url.value.trim(),
      cookieFile,
    });
    // Detect playlist vs single video
    if (info._type === "playlist" && info.entries?.length) {
      isPlaylist.value = true;
      playlistEntries.value = info.entries.map((e, i) => ({
        id: e.id || String(i + 1),
        title: e.title || `第 ${i + 1} P`,
        duration: e.duration ?? null,
        url: e.url || "",
      }));
      selectedPlaylistItems.value = playlistEntries.value.map((_, i) => i + 1);
      // Use first entry's formats if available, otherwise use playlist-level formats
      const firstEntry = info.entries[0];
      const formats: VideoFormat[] = firstEntry?.formats || info.formats || [];
      videoInfo.value = {
        ...info,
        title: info.title || firstEntry?.title || "",
        thumbnail: info.thumbnail || firstEntry?.thumbnail || "",
        duration: info.duration || firstEntry?.duration || 0,
        formats,
      };
    } else {
      isPlaylist.value = false;
      playlistEntries.value = [];
      selectedPlaylistItems.value = [];
      videoInfo.value = info;
    }

    const formats: VideoFormat[] = videoInfo.value.formats || [];
    videoFormats.value = formats
      .filter(
        (f) =>
          f.vcodec && f.vcodec !== "none" && (!f.acodec || f.acodec === "none"),
      )
      .sort((a, b) => (b.height || 0) - (a.height || 0));

    audioFormats.value = formats
      .filter(
        (f) =>
          f.acodec && f.acodec !== "none" && (!f.vcodec || f.vcodec === "none"),
      )
      .sort((a, b) => (b.abr || 0) - (a.abr || 0));

    if (videoFormats.value.length > 0)
      selectedVideoFormat.value = videoFormats.value[0].format_id;
    if (audioFormats.value.length > 0)
      selectedAudioFormat.value = audioFormats.value[0].format_id;

    detailMode.value = true;
  } catch (e: unknown) {
    window.$message.error(
      e instanceof Error ? e.message : String(e) || "获取视频信息失败",
    );
  } finally {
    fetching.value = false;
  }
};

/** 返回搜索页面，清空所有状态 */
const handleBack = () => {
  detailMode.value = false;
  videoInfo.value = null;
  videoFormats.value = [];
  audioFormats.value = [];
  selectedVideoFormat.value = "";
  selectedAudioFormat.value = "";
  downloadMode.value = "default";
  startTime.value = null;
  endTime.value = null;
  embedSubs.value = false;
  embedThumbnail.value = false;
  embedMetadata.value = false;
  noMerge.value = false;
  recodeFormat.value = "";
  limitRate.value = "";
  selectedSubtitles.value = [];
  isPlaylist.value = false;
  playlistEntries.value = [];
  selectedPlaylistItems.value = [];
  url.value = "";
};

/** 开始下载视频，未设置目录时滚动提示 */
const handleDownload = async () => {
  if (!settingStore.downloadDir) {
    window.$message.warning("请先设置下载目录");
    dirCardRef.value?.scrollIntoView({ behavior: "smooth", block: "center" });
    return;
  }

  const taskId = `dl_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`;
  const cookieFile = await getCookieFile();

  // Build format label for display
  const buildFormatLabel = (): string => {
    const parts: string[] = [];
    if (downloadMode.value === "audio") {
      parts.push("仅音频");
      const af = audioFormats.value.find(
        (f) => f.format_id === selectedAudioFormat.value,
      );
      if (af) parts.push(af.format_note || af.ext);
    } else {
      const vf = videoFormats.value.find(
        (f) => f.format_id === selectedVideoFormat.value,
      );
      if (vf) {
        if (vf.height) parts.push(`${vf.height}p`);
        if (vf.fps) parts.push(`${vf.fps}fps`);
      }
      if (downloadMode.value === "video") parts.push("仅视频");
    }
    return parts.join(" ") || "默认画质";
  };

  const dlParams = {
    url: url.value.trim(),
    downloadDir: settingStore.downloadDir,
    downloadMode: downloadMode.value,
    videoFormat: selectedVideoFormat.value || null,
    audioFormat: selectedAudioFormat.value || null,
    cookieFile,
    embedSubs: embedSubs.value,
    embedThumbnail: embedThumbnail.value,
    embedMetadata: embedMetadata.value,
    noMerge: noMerge.value,
    recodeFormat: recodeFormat.value || null,
    limitRate: limitRate.value || null,
    subtitles: selectedSubtitles.value,
    startTime: startTime.value,
    endTime: endTime.value,
    noPlaylist: isPlaylist.value && selectedPlaylistItems.value.length === 1,
    playlistItems: isPlaylist.value && selectedPlaylistItems.value.length > 0
      ? selectedPlaylistItems.value.sort((a, b) => a - b).join(",")
      : null,
  };

  downloadStore.addTask({
    id: taskId,
    url: url.value,
    title: videoInfo.value?.title || "未知视频",
    thumbnail: videoInfo.value?.thumbnail || "",
    formatLabel: buildFormatLabel(),
    status: "downloading",
    percent: 0,
    speed: "",
    eta: "",
    downloaded: "",
    total: "",
    logs: [],
    createdAt: Date.now(),
    params: dlParams,
  });

  try {
    await invoke("start_download", {
      params: { id: taskId, ...dlParams },
    });
    router.push({ name: "downloads" });
  } catch (e: unknown) {
    window.$message.error(
      e instanceof Error ? e.message : String(e) || "启动下载失败",
    );
    downloadStore.removeTask(taskId);
  }
};
</script>

<template>
  <div class="home-page">
    <Transition name="fade" mode="out-in">
      <!-- 搜索状态 -->
      <div v-if="!detailMode" key="search" class="search-view">
        <div class="search-hero">
          <div class="hero-logo">
            <icon-mdi-youtube class="hero-icon" />
            <span class="hero-text">GUI</span>
          </div>
          <n-text depth="3" style="font-size: 16px">
            粘贴视频链接，快速下载
          </n-text>
        </div>
        <div class="search-bar">
          <n-input
            v-model:value="url"
            placeholder="请输入视频链接..."
            size="large"
            round
            clearable
            :disabled="fetching"
            @keydown.enter="handleSearch"
          />
          <n-button
            type="primary"
            size="large"
            round
            strong
            secondary
            :loading="fetching"
            :disabled="!url.trim()"
            @click="handleSearch"
          >
            <template #icon>
              <n-icon>
                <icon-mdi-magnify />
              </n-icon>
            </template>
            解析
          </n-button>
        </div>
        <div class="tips-container">
          <Transition name="tip-fade" mode="out-in">
            <n-text :key="currentTipIndex" depth="3" class="tip-item">
              {{ tips[currentTipIndex] }}
            </n-text>
          </Transition>
        </div>
      </div>

      <!-- 详情状态 -->
      <div v-else key="detail" class="detail-view">
        <!-- 顶部搜索栏（只读） -->
        <div class="search-bar-mini">
          <n-button size="small" strong secondary round @click="handleBack">
            <template #icon>
              <n-icon>
                <icon-mdi-arrow-left />
              </n-icon>
            </template>
            返回
          </n-button>
          <n-input
            :value="url"
            placeholder="视频链接"
            size="small"
            round
            readonly
          />
        </div>

        <VideoInfoCard
          :video-info="videoInfo as VideoInfo"
          :is-playlist="isPlaylist"
          :playlist-count="playlistEntries.length"
          class="section-card"
        />

        <!-- 播放列表选择 -->
        <n-card v-if="isPlaylist && playlistEntries.length > 0" size="small" class="section-card">
          <template #header>
            <n-flex align="center" :size="8">
              <n-icon size="16"><icon-mdi-playlist-play /></n-icon>
              <span>播放列表</span>
              <n-tag size="small" round :bordered="false" type="info">
                {{ selectedPlaylistItems.length }} / {{ playlistEntries.length }}
              </n-tag>
            </n-flex>
          </template>
          <template #header-extra>
            <n-flex :size="8">
              <n-button size="tiny" secondary @click="selectedPlaylistItems = playlistEntries.map((_, i) => i + 1)">
                全选
              </n-button>
              <n-button size="tiny" secondary @click="selectedPlaylistItems = []">
                取消全选
              </n-button>
            </n-flex>
          </template>
          <n-checkbox-group v-model:value="selectedPlaylistItems">
            <n-flex vertical :size="6">
              <n-checkbox
                v-for="(entry, index) in playlistEntries"
                :key="entry.id"
                :value="index + 1"
                :label="`P${index + 1} ${entry.title}`"
              />
            </n-flex>
          </n-checkbox-group>
        </n-card>

        <DownloadOptionsCard
          v-model:download-mode="downloadMode"
          v-model:selected-video-format="selectedVideoFormat"
          v-model:selected-audio-format="selectedAudioFormat"
          :video-formats="videoFormats"
          :audio-formats="audioFormats"
          class="section-card"
        />

        <SubtitleCard
          :video-info="videoInfo as VideoInfo"
          v-model:selected-subtitles="selectedSubtitles"
          class="section-card"
        />

        <ExtraOptionsCard
          v-model:start-time="startTime"
          v-model:end-time="endTime"
          v-model:embed-subs="embedSubs"
          v-model:embed-thumbnail="embedThumbnail"
          v-model:embed-metadata="embedMetadata"
          v-model:no-merge="noMerge"
          v-model:recode-format="recodeFormat"
          v-model:limit-rate="limitRate"
          class="section-card"
        />

        <div ref="dirCardRef" class="section-card">
          <DownloadDirCard />
        </div>

        <!-- 底部占位 -->
        <div style="height: 64px" />
      </div>
    </Transition>

    <!-- 底部浮动下载栏 -->
    <Transition name="bottom-bar">
      <DownloadBar
        v-if="detailMode"
        :estimated-size-text="estimatedSizeText"
        @download="handleDownload"
      />
    </Transition>
  </div>
</template>

<style scoped lang="scss">
.home-page {
  height: 100%;
  position: relative;
}

// ========== 搜索视图 ==========
.search-view {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100%;
  min-height: 300px;
  gap: 20px;

  .search-hero {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;

    .hero-logo {
      display: flex;
      align-items: center;
      gap: 8px;
      user-select: none;

      .hero-icon {
        font-size: 48px;
      }

      .hero-text {
        font-weight: 800;
        font-size: 28px;
        letter-spacing: 1px;
      }
    }
  }

  .search-bar {
    display: flex;
    gap: 8px;
    width: 100%;
    max-width: 500px;
  }
}

.tips-container {
  width: 100%;
  max-width: 500px;
  text-align: center;
  height: 20px;
  position: relative;
  margin-top: -8px;

  .tip-item {
    font-size: 12px;
    display: inline-block;
  }
}

// ========== 详情视图 ==========
.detail-view {
  .section-card {
    margin-bottom: 16px;
  }
}

.search-bar-mini {
  display: flex;
  gap: 8px;
  align-items: center;
  margin-bottom: 16px;
}

</style>
