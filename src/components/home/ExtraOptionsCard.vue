<script setup lang="ts">
import { useSettingStore } from "@/stores/setting";
import {
  DEFAULT_OUTPUT_TEMPLATE,
  EXT_SUFFIX,
  TEMPLATE_ERROR_KEYS,
  normalizeOutputTemplate,
  validateOutputTemplate,
} from "@/utils/output-template";
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
const writeThumbnail = defineModel<boolean>("writeThumbnail", {
  required: true,
});
const writeDescription = defineModel<boolean>("writeDescription", {
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
const remuxFormat = defineModel<string>("remuxFormat", { required: true });
const limitRate = defineModel<string>("limitRate", { required: true });
const ffmpegArgs = defineModel<string>("ffmpegArgs", { required: true });
const customArgs = defineModel<string>("customArgs", { required: true });

/** 是否为正在直播 */
const isLive = computed(
  () => props.videoInfo.is_live === true || props.videoInfo.live_status === "is_live",
);

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

const resolvePreset = (template: string): string =>
  outputTemplatePresets.value.find((p) => p.value !== "__custom__" && p.value === template)
    ?.value ?? "__custom__";
const selectedPreset = ref(resolvePreset(settingStore.outputTemplate));

/** 模板可能在外部被修改 */
watch(
  () => settingStore.outputTemplate,
  (template) => {
    selectedPreset.value = resolvePreset(template);
  },
);

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
  // 幂等：用户手输的 .%(ext)s 不会再被追加一份
  set: (val: string) => {
    settingStore.outputTemplate = normalizeOutputTemplate(val);
  },
});

/** 当前模板的校验错误（无则为 null），用于红框提示与下载前拦截 */
const templateError = computed(() => validateOutputTemplate(settingStore.outputTemplate));

const resetTemplate = () => {
  settingStore.outputTemplate = DEFAULT_OUTPUT_TEMPLATE;
};

const customInputRef = ref<{ $el?: HTMLElement } | null>(null);

/** 快捷变量插入到光标处 */
const insertVar = (v: string) => {
  const inputEl = customInputRef.value?.$el?.querySelector("input");
  const cur = templateBase.value;
  if (!(inputEl instanceof HTMLInputElement) || inputEl.selectionStart == null) {
    templateBase.value = cur ? `${cur} ${v}` : v;
    return;
  }
  const start = inputEl.selectionStart;
  const end = inputEl.selectionEnd ?? start;
  const prefix = cur.slice(0, start);
  templateBase.value = `${prefix}${prefix && !prefix.endsWith(" ") ? " " : ""}${v}${cur.slice(end)}`;
  const caret = start + v.length + (prefix && !prefix.endsWith(" ") ? 1 : 0);
  nextTick(() => {
    inputEl.focus();
    inputEl.setSelectionRange(caret, caret);
  });
};

const recodeOptions = computed(() => [
  { label: t("detail.noConversion"), value: "" },
  { label: "MP4", value: "mp4" },
  { label: "MKV", value: "mkv" },
  { label: "WebM", value: "webm" },
  { label: "MP3", value: "mp3" },
  { label: "FLAC", value: "flac" },
]);

const remuxOptions = computed(() => [
  { label: t("common.default"), value: "" },
  { label: "MP4", value: "mp4" },
  { label: "MKV", value: "mkv" },
  { label: "WebM", value: "webm" },
  { label: "MOV", value: "mov" },
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
                ref="customInputRef"
                v-model:value="templateBase"
                placeholder="%(title).200s"
                size="small"
                :status="templateError ? 'error' : undefined"
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
            <n-text v-if="templateError" type="error" style="font-size: 12px">
              {{ t(TEMPLATE_ERROR_KEYS[templateError.code], { char: templateError.char }) }}
            </n-text>
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
        <n-flex align="center" :size="8">
          <span class="option-label">{{ $t("detail.remuxFormat") }}</span>
          <n-select
            v-model:value="remuxFormat"
            :options="remuxOptions"
            size="small"
            style="width: 110px"
          />
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
        <span class="option-label">{{ $t("detail.customArgs") }}</span>
        <n-input
          v-model:value="customArgs"
          :placeholder="$t('detail.customArgsPlaceholder')"
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
        <n-checkbox v-model:checked="writeThumbnail" size="small">
          {{ $t("detail.writeThumbnail") }}
        </n-checkbox>
        <n-checkbox v-model:checked="writeDescription" size="small">
          {{ $t("detail.writeDescription") }}
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
