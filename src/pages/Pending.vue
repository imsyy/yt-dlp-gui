<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { formatFileSize } from "@/utils/format";
import { composeOutputTemplate } from "@/utils/output-template";
import { useSettingStore } from "@/stores/setting";
import { useDownloadStore } from "@/stores/download";
import { useVideoStore } from "@/stores/video";
import { usePendingStore } from "@/stores/pending";
import { useStatusStore } from "@/stores/status";
import { useI18n } from "vue-i18n";
import type { FfmpegStatus, VideoInfo } from "@/types";
import VideoInfoCard from "@/components/home/VideoInfoCard.vue";
import DownloadOptionsCard from "@/components/home/DownloadOptionsCard.vue";
import ExtraOptionsCard from "@/components/home/ExtraOptionsCard.vue";
import SubtitleCard from "@/components/home/SubtitleCard.vue";
import DownloadDirCard from "@/components/DownloadDirCard.vue";
import DownloadBar from "@/components/home/DownloadBar.vue";

const { t } = useI18n();
const router = useRouter();
const settingStore = useSettingStore();
const downloadStore = useDownloadStore();
const videoStore = useVideoStore();
const pendingStore = usePendingStore();
const statusStore = useStatusStore();

const activeItem = computed(() => pendingStore.activeItem);

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

const estimatedSize = computed(() => {
  const item = activeItem.value;
  if (!item) return 0;
  let total = 0;
  if (item.downloadMode !== "audio") {
    const vf = item.videoFormats.find((f) => f.format_id === item.selectedVideoFormat);
    if (vf) total += vf.filesize || vf.filesize_approx || 0;
  }
  if (item.downloadMode !== "video") {
    const af = item.audioFormats.find((f) => f.format_id === item.selectedAudioFormat);
    if (af) total += af.filesize || af.filesize_approx || 0;
  }
  return total;
});

const estimatedSizeText = computed(() => {
  if (!estimatedSize.value) return t("common.unknown");
  return formatFileSize(estimatedSize.value);
});

const dirCardRef = ref<HTMLElement | null>(null);

const tabLabel = (title: string): string => {
  if (!title) return t("detail.unknownVideo");
  if (title.length > 12) return title.slice(0, 10) + "…";
  return title;
};

const handleTabClose = (name: string | number) => {
  pendingStore.remove(String(name));
};

const handleTabAdd = () => {
  router.push({ name: "home" });
};

const handleBackToHome = () => {
  router.push({ name: "home" });
};

/** 重新获取当前项视频信息 */
const handleRefresh = async () => {
  const item = activeItem.value;
  if (!item) return;
  const data = await videoStore.fetchVideoInfo(item.url);
  if (data) {
    pendingStore.refresh(item.id, data);
    window.$message.success(t("detail.refreshSuccess"));
  }
};

/** 开始下载当前项 */
const handleDownload = async () => {
  const item = activeItem.value;
  if (!item) return;

  if (!settingStore.downloadDir) {
    window.$message.warning(t("detail.setDownloadDirFirst"));
    dirCardRef.value?.scrollIntoView({ behavior: "smooth", block: "center" });
    return;
  }

  const requiresFfmpegMerge =
    item.downloadMode === "default" &&
    Boolean(item.selectedVideoFormat) &&
    Boolean(item.selectedAudioFormat) &&
    !item.noMerge;
  if (requiresFfmpegMerge) {
    try {
      const ffmpegStatus = await invoke<FfmpegStatus>("get_ffmpeg_status");
      if (!ffmpegStatus.installed) {
        statusStore.showFfmpegSetupModal = true;
        return;
      }
    } catch {
      statusStore.showFfmpegSetupModal = true;
      return;
    }
  }

  const taskId = `dl_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`;
  const { cookieFile, cookieBrowser } = await videoStore.getCookieArgs();

  const buildFormatLabel = (): string => {
    const parts: string[] = [];
    if (item.downloadMode === "audio") {
      parts.push(t("detail.audioOnly"));
      const af = item.audioFormats.find((f) => f.format_id === item.selectedAudioFormat);
      if (af) parts.push(af.format_note || af.ext);
    } else {
      const vf = item.videoFormats.find((f) => f.format_id === item.selectedVideoFormat);
      if (vf) {
        if (vf.height) parts.push(`${vf.height}p`);
        if (vf.fps) parts.push(`${vf.fps}fps`);
      }
      if (item.downloadMode === "video") parts.push(t("detail.videoOnly"));
    }
    if (item.startTime != null || item.endTime != null) {
      const s = item.startTime != null ? formatTime(timeToSeconds(item.startTime)) : "00:00";
      const e = item.endTime != null ? formatTime(timeToSeconds(item.endTime)) : t("detail.end");
      parts.push(`✂${s}-${e}`);
    }
    return parts.join(" ") || t("detail.defaultQuality");
  };

  const dlParams = {
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

  downloadStore.addTask({
    id: taskId,
    url: item.url,
    title: item.videoInfo.title || t("detail.unknownVideo"),
    thumbnail: item.videoInfo.thumbnail || "",
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

  // 任务已加入下载列表，从待下载列表移除该项
  pendingStore.remove(item.id);

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
      e instanceof Error ? e.message : String(e) || t("detail.startDownloadFailed"),
    );
    downloadStore.removeTask(taskId);
  }
};
</script>

<template>
  <div class="pending-page">
    <template v-if="pendingStore.items.length > 0">
      <n-tabs
        v-model:value="pendingStore.activeId"
        type="card"
        size="small"
        closable
        addable
        class="tabs-bar"
        @close="handleTabClose"
        @add="handleTabAdd"
      >
        <template #prefix>
          <n-button size="small" strong secondary circle @click="handleBackToHome">
            <template #icon>
              <n-icon><icon-mdi-arrow-left /></n-icon>
            </template>
          </n-button>
        </template>
        <n-tab-pane
          v-for="item in pendingStore.items"
          :key="item.id"
          :name="item.id"
          :tab="tabLabel(item.videoInfo.title)"
          display-directive="show"
        />
      </n-tabs>

      <div v-if="activeItem" :key="activeItem.id" class="pending-content">
        <n-flex :size="8" align="center" :wrap="false" style="margin-bottom: 16px">
          <n-input
            :value="activeItem.url"
            :placeholder="$t('detail.videoLink')"
            size="small"
            round
            readonly
            style="flex: 1; min-width: 0"
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
              <n-icon><icon-mdi-refresh /></n-icon>
            </template>
          </n-button>
        </n-flex>

        <VideoInfoCard
          :video-info="activeItem.videoInfo as VideoInfo"
          :is-playlist="activeItem.isPlaylist"
          :playlist-count="activeItem.playlistEntries.length"
          class="section-card"
        />

        <n-card
          v-if="activeItem.isPlaylist && activeItem.playlistEntries.length > 0"
          size="small"
          class="section-card"
        >
          <template #header>
            <n-flex align="center" :size="8">
              <n-icon size="16"><icon-mdi-playlist-play /></n-icon>
              <span>{{ $t("detail.playlist") }}</span>
              <n-tag size="small" round :bordered="false" type="info">
                {{ activeItem.selectedPlaylistItems.length }} /
                {{ activeItem.playlistEntries.length }}
              </n-tag>
            </n-flex>
          </template>
          <template #header-extra>
            <n-flex :size="8">
              <n-button
                size="tiny"
                secondary
                @click="
                  activeItem.selectedPlaylistItems = activeItem.playlistEntries.map((_, i) => i + 1)
                "
              >
                {{ $t("common.selectAll") }}
              </n-button>
              <n-button size="tiny" secondary @click="activeItem.selectedPlaylistItems = []">
                {{ $t("common.deselectAll") }}
              </n-button>
            </n-flex>
          </template>
          <n-checkbox-group v-model:value="activeItem.selectedPlaylistItems">
            <n-flex vertical :size="6">
              <n-checkbox
                v-for="(entry, index) in activeItem.playlistEntries"
                :key="entry.id"
                :value="index + 1"
                :label="`P${index + 1} ${entry.title}`"
              />
            </n-flex>
          </n-checkbox-group>
        </n-card>

        <DownloadOptionsCard
          v-model:download-mode="activeItem.downloadMode"
          v-model:selected-video-format="activeItem.selectedVideoFormat"
          v-model:selected-audio-format="activeItem.selectedAudioFormat"
          :video-formats="activeItem.videoFormats"
          :audio-formats="activeItem.audioFormats"
          :video-info="activeItem.videoInfo as VideoInfo"
          class="section-card"
        />

        <SubtitleCard
          v-model:selected-subtitles="activeItem.selectedSubtitles"
          :video-info="activeItem.videoInfo as VideoInfo"
          class="section-card"
        />

        <ExtraOptionsCard
          v-model:start-time="activeItem.startTime"
          v-model:end-time="activeItem.endTime"
          v-model:embed-subs="activeItem.embedSubs"
          v-model:embed-thumbnail="activeItem.embedThumbnail"
          v-model:embed-metadata="activeItem.embedMetadata"
          v-model:embed-chapters="activeItem.embedChapters"
          v-model:sponsorblock-remove="activeItem.sponsorblockRemove"
          v-model:extract-audio="activeItem.extractAudio"
          v-model:audio-convert-format="activeItem.audioConvertFormat"
          v-model:no-merge="activeItem.noMerge"
          v-model:recode-format="activeItem.recodeFormat"
          v-model:limit-rate="activeItem.limitRate"
          v-model:ffmpeg-args="activeItem.ffmpegArgs"
          v-model:live-from-start="activeItem.liveFromStart"
          :video-info="activeItem.videoInfo as VideoInfo"
          class="section-card"
        />

        <div ref="dirCardRef" class="section-card">
          <DownloadDirCard />
        </div>

        <DownloadBar :estimated-size-text="estimatedSizeText" @download="handleDownload" />
      </div>
    </template>

    <n-empty v-else :description="$t('pending.empty')" class="empty-state">
      <template #extra>
        <n-button type="primary" round @click="handleBackToHome">
          <template #icon>
            <n-icon><icon-mdi-magnify /></n-icon>
          </template>
          {{ $t("pending.goParse") }}
        </n-button>
      </template>
    </n-empty>
  </div>
</template>

<style scoped lang="scss">
.pending-page {
  position: relative;

  .tabs-bar {
    margin-bottom: 12px;

    :deep(.n-tabs-nav__prefix) {
      padding-right: 8px;
    }

    :deep(.n-tabs-tab) {
      padding-left: 10px;
      padding-right: 6px;
    }
  }

  .section-card {
    margin-bottom: 16px;
  }

  .pending-content {
    padding-bottom: 96px;
  }

  .empty-state {
    margin-top: 120px;
  }
}
</style>
