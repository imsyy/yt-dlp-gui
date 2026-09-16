<script setup lang="ts">
import { normalizeAudioFormat, normalizeVideoFormat } from "@/utils/normalizer";
import { getCodecKey, getCodecLabel } from "@/utils/formats";
import { useI18n } from "vue-i18n";
import type { VideoFormat, VideoInfo } from "@/types";

const { t } = useI18n();

const props = defineProps<{
  videoFormats: VideoFormat[];
  audioFormats: VideoFormat[];
  videoInfo: VideoInfo;
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

const selectedVideoCodec = ref("all");
const selectedAudioCodec = ref("all");

const createCodecOptions = (formats: VideoFormat[]) => {
  const codecs = new Map<string, string>();
  for (const format of formats) {
    const codec = format.vcodec !== "none" ? format.vcodec : format.acodec;
    codecs.set(getCodecKey(codec), getCodecLabel(codec));
  }
  return [
    { label: t("detail.allCodecs"), value: "all" },
    ...Array.from(codecs, ([value, label]) => ({ value, label })),
  ];
};

const videoCodecOptions = computed(() => createCodecOptions(props.videoFormats));
const audioCodecOptions = computed(() => createCodecOptions(props.audioFormats));

const filteredVideoFormats = computed(() =>
  selectedVideoCodec.value === "all"
    ? props.videoFormats
    : props.videoFormats.filter(
        (format) => getCodecKey(format.vcodec) === selectedVideoCodec.value,
      ),
);

const filteredAudioFormats = computed(() =>
  selectedAudioCodec.value === "all"
    ? props.audioFormats
    : props.audioFormats.filter(
        (format) => getCodecKey(format.acodec) === selectedAudioCodec.value,
      ),
);

const formatsIncomplete = computed(() => {
  if (props.audioFormats.length > 0 || props.videoFormats.length === 0) return false;
  return Math.max(...props.videoFormats.map((format) => format.height || 0)) <= 360;
});

/** 是否为正在直播 */
const isLive = computed(
  () => props.videoInfo.is_live === true || props.videoInfo.live_status === "is_live",
);

/** 视频格式下拉选项（规范化固定结构：清晰度+帧率 · 编码 · 动态范围 · 容器 · 大小 · 编号） */
const videoFormatOptions = computed(() => {
  const duration = props.videoInfo.duration || 0;
  return filteredVideoFormats.value.map((f) => {
    const n = normalizeVideoFormat(f, duration);
    const parts = [
      `${n.resolutionLabel}${n.fpsLabel ? ` ${n.fpsLabel}` : ""}`,
      n.codec,
      n.dynamicRange,
      n.container,
      n.filesizeLabel,
      `#${n.formatId}`,
    ].filter(Boolean);
    return {
      label: parts.join(" · "),
      value: n.formatId,
    };
  });
});

/** 音频格式下拉选项（规范化固定结构：[语言 · 角色] · 码率 · 编码 · 声道 · 容器 · 大小 · 编号） */
const audioFormatOptions = computed(() => {
  const duration = props.videoInfo.duration || 0;
  return filteredAudioFormats.value.map((f) => {
    const n = normalizeAudioFormat(f, duration);
    const langInfo = [n.languageLabel || n.language, n.roleLabel].filter(Boolean).join(" · ");
    const parts = [
      langInfo ? `[${langInfo}]` : "",
      n.bitrateLabel,
      n.codec,
      n.channelsLabel,
      n.container,
      n.filesizeLabel,
      `#${n.formatId}`,
    ].filter(Boolean);
    return {
      label: parts.join(" · "),
      value: n.formatId,
    };
  });
});

const handleVideoCodecChange = (value: string) => {
  selectedVideoCodec.value = value;
  const currentIsVisible = filteredVideoFormats.value.some(
    (format) => format.format_id === selectedVideoFormat.value,
  );
  if (!currentIsVisible) selectedVideoFormat.value = filteredVideoFormats.value[0]?.format_id || "";
};

const handleAudioCodecChange = (value: string) => {
  selectedAudioCodec.value = value;
  const currentIsVisible = filteredAudioFormats.value.some(
    (format) => format.format_id === selectedAudioFormat.value,
  );
  if (!currentIsVisible) selectedAudioFormat.value = filteredAudioFormats.value[0]?.format_id || "";
};

watch(
  () => props.videoFormats,
  () => {
    if (!videoCodecOptions.value.some((option) => option.value === selectedVideoCodec.value)) {
      selectedVideoCodec.value = "all";
    }
  },
);

watch(
  () => props.audioFormats,
  () => {
    if (!audioCodecOptions.value.some((option) => option.value === selectedAudioCodec.value)) {
      selectedAudioCodec.value = "all";
    }
  },
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

      <n-text
        v-if="videoFormatOptions.length === 0 && audioFormatOptions.length === 0"
        depth="3"
        class="auto-format-hint"
      >
        {{ $t("detail.autoFormatHint") }}
      </n-text>

      <n-alert v-if="formatsIncomplete" type="warning" :bordered="false">
        {{ $t("detail.incompleteFormatsHint") }}
      </n-alert>

      <n-alert v-if="isLive" type="info" :bordered="false">
        {{ $t("detail.liveFormatHint") }}
      </n-alert>

      <n-flex v-if="downloadMode !== 'audio' && videoFormatOptions.length" align="center" :size="8">
        <n-text depth="3" style="font-size: 13px; flex-shrink: 0">
          {{ $t("detail.video") }}
        </n-text>
        <n-select
          :value="selectedVideoCodec"
          :options="videoCodecOptions"
          size="small"
          style="width: 118px; flex-shrink: 0"
          :aria-label="$t('detail.codec')"
          @update:value="handleVideoCodecChange"
        />
        <n-select
          v-model:value="selectedVideoFormat"
          :options="videoFormatOptions"
          size="small"
          style="min-width: 0"
        />
      </n-flex>

      <n-flex v-if="downloadMode !== 'video' && audioFormatOptions.length" align="center" :size="8">
        <n-text depth="3" style="font-size: 13px; flex-shrink: 0">
          {{ $t("detail.audio") }}
        </n-text>
        <n-select
          :value="selectedAudioCodec"
          :options="audioCodecOptions"
          size="small"
          style="width: 118px; flex-shrink: 0"
          :aria-label="$t('detail.codec')"
          @update:value="handleAudioCodecChange"
        />
        <n-select
          v-model:value="selectedAudioFormat"
          :options="audioFormatOptions"
          size="small"
          style="min-width: 0"
        />
      </n-flex>
    </n-flex>
  </n-card>
</template>

<style scoped>
.auto-format-hint {
  font-size: 13px;
  text-wrap: pretty;
}
</style>
