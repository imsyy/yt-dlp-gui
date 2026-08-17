<script setup lang="ts">
import { formatFileSize } from "@/utils/format";
import { useVideoStore } from "@/stores/video";
import { usePendingStore } from "@/stores/pending";
import { useDownloadLauncher } from "@/composables/useDownloadLauncher";
import { useI18n } from "vue-i18n";
import type { VideoInfo } from "@/types";
import VideoInfoCard from "@/components/home/VideoInfoCard.vue";
import DownloadOptionsCard from "@/components/home/DownloadOptionsCard.vue";
import ExtraOptionsCard from "@/components/home/ExtraOptionsCard.vue";
import SubtitleCard from "@/components/home/SubtitleCard.vue";
import DownloadDirCard from "@/components/DownloadDirCard.vue";
import DownloadBar from "@/components/home/DownloadBar.vue";

const { t } = useI18n();
const router = useRouter();
const videoStore = useVideoStore();
const pendingStore = usePendingStore();
const { launchDownload } = useDownloadLauncher();

const activeItem = computed(() => pendingStore.activeItem);

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
  const result = await launchDownload(item);
  if (result === "missing-directory") {
    dirCardRef.value?.scrollIntoView({ behavior: "smooth", block: "center" });
  } else if (result === "started" || result === "queued") {
    pendingStore.remove(item.id);
    router.push({ name: "downloads" });
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
