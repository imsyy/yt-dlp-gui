<script setup lang="ts">
import { formatFileSize } from "@/utils/format";
import { useI18n } from "vue-i18n";
import type { VideoFormat } from "@/types";

const { t } = useI18n();

const props = defineProps<{
  videoFormats: VideoFormat[];
  audioFormats: VideoFormat[];
}>();

const downloadMode = defineModel<"default" | "video" | "audio">("downloadMode", {
  required: true,
});
const selectedVideoFormat = defineModel<string>("selectedVideoFormat", {
  required: true,
});
const selectedAudioFormat = defineModel<string>("selectedAudioFormat", {
  required: true,
});

/** 视频格式下拉选项 */
const videoFormatOptions = computed(() =>
  props.videoFormats.map((f) => ({
    label: `${f.height}p${f.fps ? ` ${f.fps}fps` : ""} · ${f.ext} · ${f.filesize || f.filesize_approx ? formatFileSize(f.filesize || f.filesize_approx || 0) : t("detail.unknownSize")}`,
    value: f.format_id,
  })),
);

/** 音频格式下拉选项 */
const audioFormatOptions = computed(() =>
  props.audioFormats.map((f) => ({
    label: [
      f.language ? `[${f.language}]` : "",
      f.format_note,
      f.abr ? `${f.abr}kbps` : "",
      f.acodec,
      f.audio_channels ? `${f.audio_channels}ch` : "",
      f.ext,
      f.filesize || f.filesize_approx
        ? formatFileSize(f.filesize || f.filesize_approx || 0)
        : t("detail.unknownSize"),
    ]
      .filter(Boolean)
      .filter((part, index, parts) => parts.indexOf(part) === index)
      .join(" · "),
    value: f.format_id,
  })),
);
</script>

<template>
  <n-card :title="$t('detail.downloadMethod')" size="small">
    <n-flex vertical :size="12">
      <n-radio-group v-model:value="downloadMode" size="small">
        <n-radio-button value="default">{{ $t("common.default") }}</n-radio-button>
        <n-radio-button value="video">{{ $t("detail.videoOnly") }}</n-radio-button>
        <n-radio-button value="audio">{{ $t("detail.audioOnly") }}</n-radio-button>
      </n-radio-group>

      <n-flex v-if="downloadMode !== 'audio' && videoFormatOptions.length" align="center" :size="8">
        <n-text depth="3" style="font-size: 13px; flex-shrink: 0">
          {{ $t("detail.video") }}
        </n-text>
        <n-select v-model:value="selectedVideoFormat" :options="videoFormatOptions" size="small" />
      </n-flex>

      <n-flex v-if="downloadMode !== 'video' && audioFormatOptions.length" align="center" :size="8">
        <n-text depth="3" style="font-size: 13px; flex-shrink: 0">
          {{ $t("detail.audio") }}
        </n-text>
        <n-select v-model:value="selectedAudioFormat" :options="audioFormatOptions" size="small" />
      </n-flex>
    </n-flex>
  </n-card>
</template>
