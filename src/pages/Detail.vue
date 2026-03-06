<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { formatFileSize } from "@/utils/format";
import { useSettingStore } from "@/stores/setting";
import { useDownloadStore } from "@/stores/download";
import { useVideoStore } from "@/stores/video";
import type { VideoInfo } from "@/types";
import VideoInfoCard from "@/components/home/VideoInfoCard.vue";
import DownloadOptionsCard from "@/components/home/DownloadOptionsCard.vue";
import ExtraOptionsCard from "@/components/home/ExtraOptionsCard.vue";
import SubtitleCard from "@/components/home/SubtitleCard.vue";
import DownloadDirCard from "@/components/DownloadDirCard.vue";
import DownloadBar from "@/components/home/DownloadBar.vue";

const router = useRouter();
const settingStore = useSettingStore();
const downloadStore = useDownloadStore();
const videoStore = useVideoStore();

// 无数据时返回首页
if (!videoStore.videoInfo) {
  router.replace({ name: "home" });
}

/** 将 Naive UI time picker 的时间戳值转换为当天秒数 */
const timeToSeconds = (ts: number): number => {
  const d = new Date(ts);
  return d.getHours() * 3600 + d.getMinutes() * 60 + d.getSeconds();
};

/** 秒数格式化为 HH:MM:SS */
const formatTime = (secs: number): string => {
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = secs % 60;
  const pad = (n: number) => String(n).padStart(2, "0");
  return h > 0 ? `${pad(h)}:${pad(m)}:${pad(s)}` : `${pad(m)}:${pad(s)}`;
};

// 下载模式
const downloadMode = ref<"default" | "video" | "audio">("default");

// 格式选择
const selectedVideoFormat = ref(
  videoStore.videoFormats.length > 0
    ? videoStore.videoFormats[0].format_id
    : "",
);
const selectedAudioFormat = ref(
  videoStore.audioFormats.length > 0
    ? videoStore.audioFormats[0].format_id
    : "",
);

// 额外选项
const startTime = ref<number | null>(null);
const endTime = ref<number | null>(null);
const embedSubs = ref(false);
const embedThumbnail = ref(false);
const embedMetadata = ref(false);
const embedChapters = ref(false);
const sponsorblockRemove = ref(false);
const extractAudio = ref(false);
const audioConvertFormat = ref("");
const noMerge = ref(false);
const recodeFormat = ref("");
const limitRate = ref("");
const selectedSubtitles = ref<string[]>([]);

// 下载目录卡片 ref
const dirCardRef = ref<HTMLElement | null>(null);

// ========== 计算 ==========
const estimatedSize = computed(() => {
  let total = 0;
  if (downloadMode.value !== "audio") {
    const vf = videoStore.videoFormats.find(
      (f) => f.format_id === selectedVideoFormat.value,
    );
    if (vf) total += vf.filesize || vf.filesize_approx || 0;
  }
  if (downloadMode.value !== "video") {
    const af = videoStore.audioFormats.find(
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

/** 返回搜索页面 */
const handleBack = () => {
  videoStore.clear();
  router.push({ name: "home" });
};

/** 重新获取视频信息 */
const handleRefresh = async () => {
  if (!videoStore.url) return;
  const success = await videoStore.fetchVideoInfo(videoStore.url);
  if (success) {
    selectedVideoFormat.value = videoStore.videoFormats.length > 0
      ? videoStore.videoFormats[0].format_id : "";
    selectedAudioFormat.value = videoStore.audioFormats.length > 0
      ? videoStore.audioFormats[0].format_id : "";
    window.$message.success("刷新成功");
  }
};

/** 开始下载视频 */
const handleDownload = async () => {
  if (!settingStore.downloadDir) {
    window.$message.warning("请先设置下载目录");
    dirCardRef.value?.scrollIntoView({ behavior: "smooth", block: "center" });
    return;
  }

  const taskId = `dl_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`;
  const cookieFile = await videoStore.getCookieFile();

  const buildFormatLabel = (): string => {
    const parts: string[] = [];
    if (downloadMode.value === "audio") {
      parts.push("仅音频");
      const af = videoStore.audioFormats.find(
        (f) => f.format_id === selectedAudioFormat.value,
      );
      if (af) parts.push(af.format_note || af.ext);
    } else {
      const vf = videoStore.videoFormats.find(
        (f) => f.format_id === selectedVideoFormat.value,
      );
      if (vf) {
        if (vf.height) parts.push(`${vf.height}p`);
        if (vf.fps) parts.push(`${vf.fps}fps`);
      }
      if (downloadMode.value === "video") parts.push("仅视频");
    }
    if (startTime.value != null || endTime.value != null) {
      const s = startTime.value != null ? formatTime(timeToSeconds(startTime.value)) : "00:00";
      const e = endTime.value != null ? formatTime(timeToSeconds(endTime.value)) : "结束";
      parts.push(`✂${s}-${e}`);
    }
    return parts.join(" ") || "默认画质";
  };

  const dlParams = {
    url: videoStore.url,
    downloadDir: settingStore.downloadDir,
    downloadMode: downloadMode.value,
    videoFormat: selectedVideoFormat.value || null,
    audioFormat: selectedAudioFormat.value || null,
    cookieFile,
    proxy: settingStore.proxy || null,
    outputTemplate: settingStore.outputTemplate || null,
    concurrentFragments: settingStore.concurrentFragments || null,
    noOverwrites: settingStore.noOverwrites,
    embedSubs: embedSubs.value,
    embedThumbnail: embedThumbnail.value,
    embedMetadata: embedMetadata.value,
    embedChapters: embedChapters.value,
    sponsorblockRemove: sponsorblockRemove.value,
    extractAudio: extractAudio.value,
    audioConvertFormat: audioConvertFormat.value || null,
    noMerge: noMerge.value,
    recodeFormat: recodeFormat.value || null,
    limitRate: limitRate.value || null,
    subtitles: selectedSubtitles.value,
    startTime: startTime.value != null ? timeToSeconds(startTime.value) : null,
    endTime: endTime.value != null ? timeToSeconds(endTime.value) : null,
    noPlaylist: videoStore.isPlaylist && videoStore.selectedPlaylistItems.length === 1,
    playlistItems: videoStore.isPlaylist && videoStore.selectedPlaylistItems.length > 0
      ? videoStore.selectedPlaylistItems.sort((a, b) => a - b).join(",")
      : null,
  };

  const shouldQueue = !downloadStore.canStartNow();

  downloadStore.addTask({
    id: taskId,
    url: videoStore.url,
    title: videoStore.videoInfo?.title || "未知视频",
    thumbnail: videoStore.videoInfo?.thumbnail || "",
    formatLabel: buildFormatLabel(),
    status: shouldQueue ? "queued" : "downloading",
    percent: 0,
    speed: "",
    eta: "",
    downloaded: "",
    total: "",
    logs: [],
    createdAt: Date.now(),
    params: dlParams,
  });

  if (shouldQueue) {
    router.push({ name: "downloads" });
    return;
  }

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
  <div v-if="videoStore.videoInfo" class="detail-page">
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
        :value="videoStore.url"
        placeholder="视频链接"
        size="small"
        round
        readonly
      />
      <n-button
        size="small"
        strong
        secondary
        round
        :loading="videoStore.fetching"
        @click="handleRefresh"
      >
        <template #icon>
          <n-icon>
            <icon-mdi-refresh />
          </n-icon>
        </template>
      </n-button>
    </div>

    <VideoInfoCard
      :video-info="videoStore.videoInfo as VideoInfo"
      :is-playlist="videoStore.isPlaylist"
      :playlist-count="videoStore.playlistEntries.length"
      class="section-card"
    />

    <!-- 播放列表选择 -->
    <n-card v-if="videoStore.isPlaylist && videoStore.playlistEntries.length > 0" size="small" class="section-card">
      <template #header>
        <n-flex align="center" :size="8">
          <n-icon size="16"><icon-mdi-playlist-play /></n-icon>
          <span>播放列表</span>
          <n-tag size="small" round :bordered="false" type="info">
            {{ videoStore.selectedPlaylistItems.length }} / {{ videoStore.playlistEntries.length }}
          </n-tag>
        </n-flex>
      </template>
      <template #header-extra>
        <n-flex :size="8">
          <n-button size="tiny" secondary @click="videoStore.selectedPlaylistItems = videoStore.playlistEntries.map((_, i) => i + 1)">
            全选
          </n-button>
          <n-button size="tiny" secondary @click="videoStore.selectedPlaylistItems = []">
            取消全选
          </n-button>
        </n-flex>
      </template>
      <n-checkbox-group v-model:value="videoStore.selectedPlaylistItems">
        <n-flex vertical :size="6">
          <n-checkbox
            v-for="(entry, index) in videoStore.playlistEntries"
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
      :video-formats="videoStore.videoFormats"
      :audio-formats="videoStore.audioFormats"
      class="section-card"
    />

    <SubtitleCard
      v-model:selected-subtitles="selectedSubtitles"
      :video-info="videoStore.videoInfo as VideoInfo"
      class="section-card"
    />

    <ExtraOptionsCard
      v-model:start-time="startTime"
      v-model:end-time="endTime"
      v-model:embed-subs="embedSubs"
      v-model:embed-thumbnail="embedThumbnail"
      v-model:embed-metadata="embedMetadata"
      v-model:embed-chapters="embedChapters"
      v-model:sponsorblock-remove="sponsorblockRemove"
      v-model:extract-audio="extractAudio"
      v-model:audio-convert-format="audioConvertFormat"
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

    <!-- 底部浮动下载栏 -->
    <DownloadBar
      :estimated-size-text="estimatedSizeText"
      @download="handleDownload"
    />
  </div>
</template>

<style scoped lang="scss">
.detail-page {
  position: relative;

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
