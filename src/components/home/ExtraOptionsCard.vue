<script setup lang="ts">
import { useSettingStore } from "@/stores/setting";
import { DEFAULT_OUTPUT_TEMPLATE } from "@/utils/output-template";
import { useI18n } from "vue-i18n";
import type { VideoInfo } from "@/types";

const { t } = useI18n();
const settingStore = useSettingStore();

const props = defineProps<{
  videoInfo: VideoInfo;
}>();

const startTime = defineModel<number | null>("startTime", {
  required: true,
});
const endTime = defineModel<number | null>("endTime", {
  required: true,
});
const embedSubs = defineModel<boolean>("embedSubs", { required: true });
const embedThumbnail = defineModel<boolean>("embedThumbnail", {
  required: true,
});
const embedMetadata = defineModel<boolean>("embedMetadata", {
  required: true,
});
const embedChapters = defineModel<boolean>("embedChapters", {
  required: true,
});
const sponsorblockRemove = defineModel<boolean>("sponsorblockRemove", {
  required: true,
});
const extractAudio = defineModel<boolean>("extractAudio", {
  required: true,
});
const audioConvertFormat = defineModel<string>("audioConvertFormat", {
  required: true,
});
const noMerge = defineModel<boolean>("noMerge", { required: true });
const recodeFormat = defineModel<string>("recodeFormat", { required: true });
const limitRate = defineModel<string>("limitRate", { required: true });
const ffmpegArgs = defineModel<string>("ffmpegArgs", { required: true });
const liveFromStart = defineModel<boolean>("liveFromStart", { required: true });

/** 是否为正在直播 */
const isLive = computed(
  () => props.videoInfo.is_live === true || props.videoInfo.live_status === "is_live",
);
/** 是否为预约直播 */
const isUpcoming = computed(() => props.videoInfo.live_status === "is_upcoming");
/** 是否为直播相关（正在直播或预约） */
const isLiveRelated = computed(() => isLive.value || isUpcoming.value);

const outputTemplatePresets = computed(() => [
  { label: t("common.default"), value: DEFAULT_OUTPUT_TEMPLATE },
  { label: t("detail.titleQuality"), value: "%(title).200s [%(height)sp].%(ext)s" },
  { label: t("detail.authorTitle"), value: "%(uploader)s - %(title).200s.%(ext)s" },
  { label: t("detail.dateTitle"), value: "%(upload_date)s - %(title).200s.%(ext)s" },
  { label: t("detail.titleId"), value: "%(title).200s [%(id)s].%(ext)s" },
  { label: t("detail.custom"), value: "__custom__" },
]);

const templateVars = computed(() => [
  { label: t("detail.tplTitle"), value: "%(title)s" },
  { label: t("detail.tplAuthor"), value: "%(uploader)s" },
  { label: t("detail.tplDate"), value: "%(upload_date)s" },
  { label: t("detail.tplId"), value: "%(id)s" },
  { label: t("detail.tplQuality"), value: "%(height)sp" },
  { label: t("detail.tplResolution"), value: "%(resolution)s" },
  { label: t("detail.tplDuration"), value: "%(duration)s" },
]);

const EXT_SUFFIX = ".%(ext)s";

const getInitialPreset = () => {
  const cur = settingStore.outputTemplate;
  const match = outputTemplatePresets.value.find(
    (p) => p.value !== "__custom__" && p.value === cur,
  );
  return match ? cur : "__custom__";
};
const selectedPreset = ref(getInitialPreset());

const customMode = computed(() => selectedPreset.value === "__custom__");

const handleTemplateSelect = (val: string) => {
  selectedPreset.value = val;
  if (val !== "__custom__") {
    settingStore.outputTemplate = val;
  }
};

const templateBase = computed({
  get: () => {
    const cur = settingStore.outputTemplate;
    return cur.endsWith(EXT_SUFFIX) ? cur.slice(0, -EXT_SUFFIX.length) : cur;
  },
  set: (val: string) => {
    settingStore.outputTemplate = val + EXT_SUFFIX;
  },
});

const resetTemplate = () => {
  settingStore.outputTemplate = DEFAULT_OUTPUT_TEMPLATE;
};

const insertVar = (v: string) => {
  templateBase.value = templateBase.value + " " + v;
};

const recodeOptions = computed(() => [
  { label: t("detail.noConversion"), value: "" },
  { label: "MP4", value: "mp4" },
  { label: "MKV", value: "mkv" },
  { label: "WebM", value: "webm" },
  { label: "MP3", value: "mp3" },
  { label: "FLAC", value: "flac" },
]);

const limitRateOptions = computed(() => [
  { label: t("detail.noLimit"), value: "" },
  { label: "500K/s", value: "500K" },
  { label: "1M/s", value: "1M" },
  { label: "2M/s", value: "2M" },
  { label: "5M/s", value: "5M" },
  { label: "10M/s", value: "10M" },
]);

const audioConvertOptions = computed(() => [
  { label: t("detail.noConversion"), value: "" },
  { label: "MP3", value: "mp3" },
  { label: "FLAC", value: "flac" },
  { label: "WAV", value: "wav" },
  { label: "AAC", value: "aac" },
  { label: "OPUS", value: "opus" },
  { label: "M4A", value: "m4a" },
]);

/** 开始时间变化时，若结束时间未选择或早于等于开始时间则自动设为开始时间 + 1 分钟 */
watch(startTime, (val) => {
  if (val != null && (endTime.value == null || endTime.value <= val)) {
    endTime.value = val + 60000;
  }
});

/** 结束时间变化时，若早于等于开始时间则自动修正为开始时间 + 1 分钟 */
watch(endTime, (val) => {
  if (val != null && startTime.value != null && val <= startTime.value) {
    endTime.value = startTime.value + 60000;
    window.$message.warning(t("detail.endTimeAdjusted"));
  }
});
</script>

<template>
  <n-card :title="$t('detail.extraOptions')" size="small">
    <n-flex vertical :size="14">
      <n-flex align="center" :size="8">
        <span class="option-label">{{ $t("detail.filename") }}</span>
        <n-flex vertical :size="6" style="flex: 1; min-width: 0">
          <n-select
            :value="selectedPreset"
            :options="outputTemplatePresets"
            size="small"
            @update:value="handleTemplateSelect"
          />
          <template v-if="customMode">
            <n-flex align="center" :size="6">
              <n-input
                v-model:value="templateBase"
                placeholder="%(title).200s"
                size="small"
                style="flex: 1"
              >
                <template #suffix>
                  <n-text depth="3" style="font-size: 12px; white-space: nowrap">.%(ext)s</n-text>
                </template>
              </n-input>
              <n-button size="small" secondary @click="resetTemplate">
                <template #icon>
                  <n-icon size="14"><icon-mdi-refresh /></n-icon>
                </template>
              </n-button>
            </n-flex>
            <n-flex :size="6" wrap>
              <n-tag
                v-for="v in templateVars"
                :key="v.value"
                size="small"
                round
                :bordered="false"
                style="cursor: pointer"
                @click="insertVar(v.value)"
              >
                {{ v.label }}
              </n-tag>
            </n-flex>
          </template>
          <n-input-group>
            <n-input
              v-model:value="settingStore.filenamePrefix"
              :placeholder="$t('detail.filenamePrefix')"
              size="small"
              clearable
            />
            <n-input
              v-model:value="settingStore.filenameSuffix"
              :placeholder="$t('detail.filenameSuffix')"
              size="small"
              clearable
            />
          </n-input-group>
        </n-flex>
      </n-flex>

      <n-flex align="center" :size="8">
        <span class="option-label">{{ $t("detail.timeTrim") }}</span>
        <n-flex align="center" :size="8">
          <n-time-picker
            v-model:value="startTime"
            :placeholder="$t('detail.start')"
            size="small"
            clearable
            format="HH:mm:ss"
            style="width: 120px"
            :actions="[]"
            :disabled="isLive"
          />
          <n-text depth="3">—</n-text>
          <n-time-picker
            v-model:value="endTime"
            :placeholder="$t('detail.end')"
            size="small"
            clearable
            format="HH:mm:ss"
            style="width: 120px"
            :actions="[]"
            :disabled="isLive"
          />
          <n-text v-if="isLive" depth="3" style="font-size: 12px">
            {{ $t("detail.liveTimeTrimDisabled") }}
          </n-text>
        </n-flex>
      </n-flex>

      <n-flex :size="16" wrap>
        <n-flex v-if="isLiveRelated" align="center" :size="8">
          <n-checkbox v-model:checked="liveFromStart" size="small" :disabled="isUpcoming">
            {{ $t("detail.liveFromStart") }}
          </n-checkbox>
        </n-flex>
        <n-flex align="center" :size="8">
          <span class="option-label">{{ $t("detail.recodeFormat") }}</span>
          <n-select
            v-model:value="recodeFormat"
            :options="recodeOptions"
            size="small"
            style="width: 110px"
          />
        </n-flex>
        <n-flex align="center" :size="8">
          <span class="option-label">{{ $t("detail.speedLimit") }}</span>
          <n-select
            v-model:value="limitRate"
            :options="limitRateOptions"
            size="small"
            style="width: 110px"
          />
        </n-flex>
      </n-flex>

      <n-flex align="center" :size="8">
        <span class="option-label">{{ $t("detail.ffmpegArgs") }}</span>
        <n-input
          v-model:value="ffmpegArgs"
          :placeholder="$t('detail.ffmpegArgsPlaceholder')"
          size="small"
          clearable
          style="flex: 1"
        />
      </n-flex>

      <n-flex align="center" :size="8">
        <n-checkbox v-model:checked="extractAudio" size="small">
          {{ $t("detail.extractAudio") }}
        </n-checkbox>
        <n-select
          v-model:value="audioConvertFormat"
          :options="audioConvertOptions"
          :style="{ visibility: extractAudio ? 'visible' : 'hidden' }"
          size="small"
          style="width: 110px"
          :placeholder="$t('detail.audioFormat')"
        />
      </n-flex>

      <n-divider style="margin: 0" />

      <n-flex :size="[16, 8]" wrap>
        <n-checkbox v-model:checked="embedSubs" size="small">
          {{ $t("detail.embedSubs") }}
        </n-checkbox>
        <n-checkbox v-model:checked="embedThumbnail" size="small">
          {{ $t("detail.embedThumbnail") }}
        </n-checkbox>
        <n-checkbox v-model:checked="embedMetadata" size="small">
          {{ $t("detail.embedMetadata") }}
        </n-checkbox>
        <n-checkbox v-model:checked="embedChapters" size="small">
          {{ $t("detail.embedChapters") }}
        </n-checkbox>
        <n-checkbox v-model:checked="sponsorblockRemove" size="small">
          {{ $t("detail.skipSponsor") }}
        </n-checkbox>
        <n-checkbox v-model:checked="noMerge" size="small">
          {{ $t("detail.noMerge") }}
        </n-checkbox>
      </n-flex>
    </n-flex>
  </n-card>
</template>

<style scoped lang="scss">
.option-label {
  font-size: 13px;
  color: var(--n-text-color-3, #999);
  flex-shrink: 0;
  min-width: 56px;
}
</style>
