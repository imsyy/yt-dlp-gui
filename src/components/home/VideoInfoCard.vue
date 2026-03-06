<script setup lang="ts">
import type { VideoInfo } from "@/types";

const props = defineProps<{
  videoInfo: VideoInfo;
  isPlaylist?: boolean;
  playlistCount?: number;
}>();

/** 格式化时长为 h:mm:ss 或 m:ss */
const formatDuration = (seconds: number): string => {
  if (!seconds) return "0:00";
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = Math.floor(seconds % 60);
  if (h > 0)
    return `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
  return `${m}:${String(s).padStart(2, "0")}`;
};

/** 格式化播放次数，超过万/亿使用中文单位 */
const formatViewCount = (count: number): string => {
  if (!count) return "";
  if (count >= 100000000) return `${(count / 100000000).toFixed(1)}亿`;
  if (count >= 10000) return `${(count / 10000).toFixed(1)}万`;
  return String(count);
};

const coverError = ref(false);
const coverLoaded = ref(false);

watch(
  () => props.videoInfo.thumbnail,
  () => {
    coverError.value = false;
    coverLoaded.value = false;
  },
);
</script>

<template>
  <n-card size="small">
    <div class="video-info">
      <div class="video-cover">
        <n-skeleton
          v-if="!coverLoaded && !coverError"
          width="200"
          height="112"
          style="border-radius: 6px"
        />
        <div v-if="coverError" class="cover-fallback">
          <icon-mdi-image-broken-variant style="font-size: 32px; opacity: 0.4" />
        </div>
        <img
          v-show="coverLoaded && !coverError"
          :src="videoInfo.thumbnail"
          class="cover-img"
          @load="coverLoaded = true"
          @error="coverError = true"
        />
        <div v-if="coverLoaded && !coverError" class="video-duration">
          {{ formatDuration(videoInfo.duration) }}
        </div>
      </div>
      <div class="video-meta">
        <n-flex align="center" :size="8" style="margin-bottom: 4px">
          <n-tag v-if="isPlaylist" size="small" round :bordered="false" type="warning">
            <template #icon>
              <n-icon size="12"><icon-mdi-playlist-play /></n-icon>
            </template>
            合集 {{ playlistCount }}P
          </n-tag>
        </n-flex>
        <n-ellipsis :line-clamp="2" :tooltip="false" class="video-title">
          {{ videoInfo.title }}
        </n-ellipsis>
        <n-flex :size="12" align="center" style="margin-top: 8px">
          <n-text depth="3" style="font-size: 12px">
            <icon-mdi-account
              style="vertical-align: -2px; margin-right: 2px"
            />
            {{ videoInfo.uploader || "未知" }}
          </n-text>
          <n-text
            v-if="videoInfo.view_count"
            depth="3"
            style="font-size: 12px"
          >
            <icon-mdi-eye
              style="vertical-align: -2px; margin-right: 2px"
            />
            {{ formatViewCount(videoInfo.view_count) }}次播放
          </n-text>
        </n-flex>
      </div>
    </div>
  </n-card>
</template>

<style scoped lang="scss">
.video-info {
  display: flex;
  gap: 16px;
}

.video-cover {
  position: relative;
  flex-shrink: 0;
  width: 200px;
  min-height: 112px;

  .cover-img {
    width: 200px;
    height: auto;
    border-radius: 6px;
    object-fit: cover;
    display: block;
  }

  .cover-fallback {
    width: 200px;
    height: 112px;
    border-radius: 6px;
    background: var(--n-color-modal);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .video-duration {
    position: absolute;
    bottom: 6px;
    right: 6px;
    background: rgba(0, 0, 0, 0.5);
    color: #fff;
    font-size: 11px;
    padding: 1px 5px;
    border-radius: 3px;
    font-variant-numeric: tabular-nums;
  }
}

.video-meta {
  flex: 1;
  min-width: 0;

  .video-title {
    font-size: 15px;
    font-weight: 600;
    line-height: 1.4;
  }
}
</style>
