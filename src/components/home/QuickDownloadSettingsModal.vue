<script setup lang="ts">
import { useSettingStore } from "@/stores/setting";
import DownloadDirCard from "@/components/DownloadDirCard.vue";

const show = defineModel<boolean>("show", { required: true });
const settingStore = useSettingStore();

const qualityOptions = [
  { label: "2160p", value: 2160 },
  { label: "1440p", value: 1440 },
  { label: "1080p", value: 1080 },
  { label: "720p", value: 720 },
  { label: "480p", value: 480 },
  { label: "360p", value: 360 },
];

const recodeOptions = [
  { label: "—", value: "" },
  { label: "MP4", value: "mp4" },
  { label: "MKV", value: "mkv" },
  { label: "WebM", value: "webm" },
  { label: "MP3", value: "mp3" },
  { label: "FLAC", value: "flac" },
];

const limitRateOptions = [
  { label: "—", value: "" },
  { label: "500K/s", value: "500K" },
  { label: "1M/s", value: "1M" },
  { label: "2M/s", value: "2M" },
  { label: "5M/s", value: "5M" },
  { label: "10M/s", value: "10M" },
];
</script>

<template>
  <n-modal v-model:show="show">
    <n-card
      :title="$t('home.quickSettings')"
      size="small"
      role="dialog"
      aria-modal="true"
      class="quick-settings-card"
    >
      <n-scrollbar class="quick-settings-scroll">
        <n-flex vertical :size="14" class="quick-settings-content">
          <DownloadDirCard />

          <n-flex align="center" :size="8">
            <span class="option-label">{{ $t("detail.downloadMethod") }}</span>
            <n-radio-group v-model:value="settingStore.quickDownloadMode" size="small">
              <n-radio-button value="default">{{ $t("common.default") }}</n-radio-button>
              <n-radio-button value="video">{{ $t("detail.videoOnly") }}</n-radio-button>
              <n-radio-button value="audio">{{ $t("detail.audioOnly") }}</n-radio-button>
            </n-radio-group>
          </n-flex>

          <n-flex
            v-if="settingStore.quickDownloadMode !== 'audio'"
            align="center"
            :size="8"
          >
            <span class="option-label">{{ $t("home.maxQuality") }}</span>
            <n-select
              v-model:value="settingStore.quickMaxHeight"
              :options="qualityOptions"
              size="small"
              class="compact-select"
            />
          </n-flex>

          <n-flex :size="16" wrap>
            <n-flex align="center" :size="8">
              <span class="option-label">{{ $t("detail.recodeFormat") }}</span>
              <n-select
                v-model:value="settingStore.quickRecodeFormat"
                :options="recodeOptions"
                size="small"
                class="compact-select"
              />
            </n-flex>
            <n-flex align="center" :size="8">
              <span class="option-label">{{ $t("detail.speedLimit") }}</span>
              <n-select
                v-model:value="settingStore.quickLimitRate"
                :options="limitRateOptions"
                size="small"
                class="compact-select"
              />
            </n-flex>
          </n-flex>

          <n-flex align="center" :size="8" :wrap="false">
            <span class="option-label">{{ $t("detail.ffmpegArgs") }}</span>
            <n-input
              v-model:value="settingStore.quickFfmpegArgs"
              :placeholder="$t('detail.ffmpegArgsPlaceholder')"
              size="small"
              clearable
              class="ffmpeg-input"
            />
          </n-flex>

          <n-divider style="margin: 0" />

          <n-flex :size="[16, 8]" wrap>
            <n-checkbox v-model:checked="settingStore.quickEmbedThumbnail" size="small">
              {{ $t("detail.embedThumbnail") }}
            </n-checkbox>
            <n-checkbox v-model:checked="settingStore.quickEmbedMetadata" size="small">
              {{ $t("detail.embedMetadata") }}
            </n-checkbox>
            <n-checkbox v-model:checked="settingStore.quickEmbedChapters" size="small">
              {{ $t("detail.embedChapters") }}
            </n-checkbox>
            <n-checkbox v-model:checked="settingStore.quickSponsorblockRemove" size="small">
              {{ $t("detail.skipSponsor") }}
            </n-checkbox>
            <n-checkbox v-model:checked="settingStore.quickNoMerge" size="small">
              {{ $t("detail.noMerge") }}
            </n-checkbox>
          </n-flex>
        </n-flex>
      </n-scrollbar>

      <template #footer>
        <n-flex justify="end">
          <n-button size="small" type="primary" secondary @click="show = false">
            {{ $t("common.save") }}
          </n-button>
        </n-flex>
      </template>
    </n-card>
  </n-modal>
</template>

<style scoped lang="scss">
.quick-settings-card {
  width: min(480px, calc(100vw - 32px));
}

.quick-settings-scroll {
  max-height: min(520px, calc(100vh - 120px));
}

.quick-settings-content {
  padding-right: 8px;
}

.option-label {
  min-width: 56px;
  flex-shrink: 0;
  color: var(--n-text-color-3, #999);
  font-size: 13px;
  white-space: nowrap;
}

.compact-select {
  width: 110px;
}

.ffmpeg-input {
  min-width: 0;
  flex: 1;
}
</style>
