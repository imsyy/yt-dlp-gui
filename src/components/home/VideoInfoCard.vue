<script setup lang="ts">
import { useI18n } from "vue-i18n";
import type { VideoInfo } from "@/types";

const { t } = useI18n();

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
  if (h > 0) return `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
  return `${m}:${String(s).padStart(2, "0")}`;
};

/** 格式化播放次数 */
const formatViewCount = (count: number): string => {
  if (!count) return "";
  if (count >= 100000000) return `${(count / 100000000).toFixed(1)}${t("detail.viewYi")}`;
  if (count >= 10000) return `${(count / 10000).toFixed(1)}${t("detail.viewWan")}`;
  return String(count);
};

/** 是否为正在直播 */
const isLive = computed(
  () => props.videoInfo.is_live === true || props.videoInfo.live_status === "is_live",
);
/** 是否为预约直播（尚未开始） */
const isUpcoming = computed(() => props.videoInfo.live_status === "is_upcoming");

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
    <n-flex :size="16" :wrap="false">
      <div class="video-cover">
        <div v-if="!coverLoaded || coverError" class="cover-placeholder" aria-hidden="true">
          <icon-mdi-image-broken-variant v-if="coverError" />
          <icon-mdi-image-outline v-else />
        </div>
        <img
          v-if="videoInfo.thumbnail && !coverError"
          :src="videoInfo.thumbnail"
          class="cover-img"
          :class="{ 'cover-img-loading': !coverLoaded }"
          alt=""
          @load="coverLoaded = true"
          @error="coverError = true"
        />
        <div v-if="coverLoaded && !coverError && !isLive" class="video-duration">
          {{ formatDuration(videoInfo.duration) }}
        </div>
        <div v-if="coverLoaded && !coverError && isLive" class="video-live-badge">
          <span class="live-dot" />
          {{ $t("detail.liveNow") }}
        </div>
      </div>
      <div class="video-meta">
        <n-flex align="center" :size="8" style="margin-bottom: 4px">
          <n-tag v-if="isPlaylist" size="small" round :bordered="false" type="warning">
            <template #icon>
              <n-icon size="12"><icon-mdi-playlist-play /></n-icon>
            </template>
            {{ $t("detail.playlistTag", { n: playlistCount }) }}
          </n-tag>
          <n-tag v-if="isUpcoming" size="small" round :bordered="false" type="warning">
            {{ $t("detail.liveUpcoming") }}
          </n-tag>
        </n-flex>
        <n-ellipsis :line-clamp="2" :tooltip="false" class="video-title">
          {{ videoInfo.title }}
        </n-ellipsis>
        <n-flex :size="12" align="center" style="margin-top: 8px">
          <n-text depth="3" style="font-size: 12px">
            <icon-mdi-account style="vertical-align: -2px; margin-right: 2px" />
            {{ videoInfo.uploader || $t("common.unknown") }}
          </n-text>
          <n-text v-if="videoInfo.view_count" depth="3" style="font-size: 12px">
            <icon-mdi-eye style="vertical-align: -2px; margin-right: 2px" />
            {{ formatViewCount(videoInfo.view_count) }}{{ $t("detail.views") }}
          </n-text>
        </n-flex>
      </div>
    </n-flex>
  </n-card>
</template>

<style scoped lang="scss">
.video-cover {
  position: relative;
  flex-shrink: 0;
  width: 200px;
  aspect-ratio: 16 / 9;
  overflow: hidden;
  border-radius: 6px;
  background: var(--n-color-modal);

  .cover-img {
    width: 100%;
    height: 100%;
    display: block;
    object-fit: cover;
  }

  .cover-img-loading {
    opacity: 0;
  }

  .cover-placeholder {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    font-size: 32px;
    opacity: 0.4;
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

  .video-live-badge {
    position: absolute;
    bottom: 6px;
    right: 6px;
    background: rgba(220, 38, 38, 0.85);
    color: #fff;
    font-size: 11px;
    padding: 1px 6px;
    border-radius: 3px;
    display: flex;
    align-items: center;
    gap: 4px;
    font-weight: 600;
  }

  .live-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: #fff;
    animation: live-pulse 1.5s ease-in-out infinite;
  }

  @keyframes live-pulse {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.3;
    }
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
