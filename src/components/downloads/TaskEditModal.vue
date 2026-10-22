<script setup lang="ts">
import { open } from "@tauri-apps/plugin-dialog";
import { useI18n } from "vue-i18n";
import { useDownloadStore } from "@/stores/download";
import { normalizeTaskParams } from "@/utils/taskParams";
import { isValidTimeRange } from "@/utils/validate";
import { TEMPLATE_ERROR_KEYS, validateOutputTemplate } from "@/utils/output-template";
import { formatError } from "@/utils/format";
import type { DownloadTask } from "@/types";

const props = defineProps<{ task: DownloadTask }>();
const show = defineModel<boolean>("show", { required: true });
const { t } = useI18n();
const downloadStore = useDownloadStore();
const loading = ref(false);
const cloneParams = () => {
  const params = normalizeTaskParams(props.task.params);
  return { ...params, subtitles: [...params.subtitles] };
};
const draft = ref(cloneParams());
let editingId = props.task.id;

watch(show, (visible) => {
  if (visible) {
    draft.value = cloneParams();
    editingId = props.task.id;
  }
});

const modeOptions = computed(() => [
  { label: t("common.default"), value: "default" },
  { label: t("detail.videoOnly"), value: "video" },
  { label: t("detail.audioOnly"), value: "audio" },
]);
const conversionFields = computed(() => [
  { key: "audioConvertFormat" as const, label: t("detail.audioFormat") },
  { key: "recodeFormat" as const, label: t("detail.recodeFormat") },
  { key: "remuxFormat" as const, label: t("detail.remuxFormat") },
  { key: "limitRate" as const, label: t("detail.speedLimit") },
]);
const templateError = computed(() =>
  draft.value.outputTemplate ? validateOutputTemplate(draft.value.outputTemplate) : null,
);
const timeValid = computed(() => isValidTimeRange(draft.value.startTime, draft.value.endTime));
const audioValid = computed(
  () =>
    (!draft.value.extractAudio && draft.value.downloadMode !== "audio") ||
    !draft.value.audioFormat?.includes("+"),
);
const valid = computed(
  () =>
    Boolean(draft.value.downloadDir.trim()) &&
    timeValid.value &&
    audioValid.value &&
    !templateError.value,
);

const updateVideoFormat = (value: string) => {
  draft.value.videoFormat = value || null;
  draft.value.videoHasAudio =
    draft.value.videoFormat === props.task.params.videoFormat &&
    Boolean(props.task.params.videoHasAudio);
};

const selectDirectory = async () => {
  try {
    const selected = await open({
      directory: true,
      multiple: false,
      title: t("downloadDir.selectDir"),
    });
    if (typeof selected === "string") draft.value.downloadDir = selected;
  } catch (error: unknown) {
    window.$message.error(formatError(error instanceof Error ? error.message : String(error)));
  }
};

const handleSubmit = async () => {
  if (!valid.value || loading.value) return;
  loading.value = true;
  try {
    const accepted = await downloadStore.retryTask(editingId, draft.value);
    if (accepted) show.value = false;
    else window.$message.error(formatError(props.task.error || t("downloads.retryFailed")));
  } catch (error: unknown) {
    window.$message.error(formatError(error instanceof Error ? error.message : String(error)));
  } finally {
    loading.value = false;
  }
};
</script>

<template>
  <n-modal
    v-model:show="show"
    preset="card"
    :title="$t('downloads.editAndRetry')"
    size="small"
    :bordered="false"
    content-scrollable
    :segmented="{ action: 'soft' }"
    :style="{ width: '680px', maxWidth: '94vw', maxHeight: '85vh' }"
    :closable="!loading"
    :mask-closable="!loading"
    :close-on-esc="!loading"
  >
    <n-flex vertical :size="14">
      <n-text depth="3">{{ $t("downloads.editTaskHint") }}</n-text>
      <n-text>{{ task.title || task.url }}</n-text>
      <n-form size="small" label-placement="top" :disabled="loading" :show-feedback="false">
        <n-flex vertical :size="14">
          <n-form-item :label="$t('downloadDir.title')" :required="true">
            <n-input-group>
              <n-input
                v-model:value="draft.downloadDir"
                :status="draft.downloadDir.trim() ? undefined : 'error'"
              />
              <n-button @click="selectDirectory">{{ $t("common.select") }}</n-button>
            </n-input-group>
          </n-form-item>
          <n-form-item :label="$t('detail.filename')">
            <n-input
              :value="draft.outputTemplate"
              :placeholder="$t('common.default')"
              :status="templateError ? 'error' : undefined"
              @update:value="draft.outputTemplate = $event || null"
            />
          </n-form-item>
          <n-text v-if="templateError" type="error">
            {{ t(TEMPLATE_ERROR_KEYS[templateError.code], { char: templateError.char }) }}
          </n-text>
          <n-form-item :label="$t('detail.downloadMethod')">
            <n-select v-model:value="draft.downloadMode" :options="modeOptions" />
          </n-form-item>
          <div class="edit-grid">
            <n-form-item :label="$t('downloads.videoFormatId')">
              <n-input
                :value="draft.videoFormat"
                :placeholder="$t('common.default')"
                @update:value="updateVideoFormat"
              />
            </n-form-item>
            <n-form-item :label="$t('downloads.audioFormatIds')">
              <n-input
                :value="draft.audioFormat"
                :placeholder="$t('common.default')"
                :status="audioValid ? undefined : 'error'"
                @update:value="draft.audioFormat = $event || null"
              />
            </n-form-item>
          </div>
          <n-text v-if="!audioValid" type="error">{{ $t("downloads.editSingleAudio") }}</n-text>
          <n-text depth="3">{{ $t("downloads.editTimeHint") }}</n-text>
          <div class="edit-grid">
            <n-form-item :label="$t('detail.start')">
              <n-input-number
                v-model:value="draft.startTime"
                :min="0"
                clearable
                :status="timeValid ? undefined : 'error'"
                style="width: 100%"
              />
            </n-form-item>
            <n-form-item :label="$t('detail.end')">
              <n-input-number
                v-model:value="draft.endTime"
                :min="0"
                clearable
                :status="timeValid ? undefined : 'error'"
                style="width: 100%"
              />
            </n-form-item>
          </div>
          <n-text v-if="!timeValid" type="error">{{ $t("downloads.invalidEditTimeRange") }}</n-text>
          <n-checkbox
            v-model:checked="draft.forceKeyframesAtCuts"
            :disabled="draft.startTime == null && draft.endTime == null"
          >
            {{ $t("detail.forceKeyframesAtCuts") }}
          </n-checkbox>
          <n-text depth="3">{{ $t("detail.forceKeyframesAtCutsHint") }}</n-text>
          <n-flex :size="16">
            <n-checkbox v-model:checked="draft.noMerge">{{ $t("detail.noMerge") }}</n-checkbox>
            <n-checkbox v-model:checked="draft.extractAudio">
              {{ $t("detail.extractAudio") }}
            </n-checkbox>
          </n-flex>
          <div class="edit-grid">
            <n-form-item v-for="field in conversionFields" :key="field.key" :label="field.label">
              <n-input
                :value="draft[field.key]"
                :placeholder="$t('common.default')"
                @update:value="draft[field.key] = $event || null"
              />
            </n-form-item>
          </div>
          <n-form-item :label="$t('detail.ffmpegArgs')">
            <n-input :value="draft.ffmpegArgs" @update:value="draft.ffmpegArgs = $event || null" />
          </n-form-item>
          <n-form-item :label="$t('detail.customArgs')">
            <n-input
              :value="draft.customArgs"
              type="textarea"
              :autosize="{ minRows: 2, maxRows: 6 }"
              @update:value="draft.customArgs = $event || null"
            />
          </n-form-item>
          <n-form-item :label="$t('settings.proxy')">
            <n-input
              :value="draft.proxy"
              type="password"
              show-password-on="click"
              clearable
              @update:value="draft.proxy = $event || null"
            />
          </n-form-item>
          <n-collapse>
            <n-collapse-item :title="$t('downloads.retainedOptions')" name="original">
              <n-flex vertical :size="8">
                <n-text>{{ $t("cookie.file") }}: {{ draft.cookieFile || "—" }}</n-text>
                <n-text>{{ $t("cookie.browser") }}: {{ draft.cookieBrowser || "—" }}</n-text>
                <n-text>
                  {{ $t("detail.subtitles") }}: {{ draft.subtitles.join(", ") || "—" }}
                </n-text>
                <n-text>
                  {{ $t("detail.playlist") }}:
                  {{ draft.noPlaylist ? "—" : draft.playlistItems || $t("common.selectAll") }}
                </n-text>
              </n-flex>
            </n-collapse-item>
          </n-collapse>
        </n-flex>
      </n-form>
    </n-flex>
    <template #action>
      <n-flex justify="end" :size="8">
        <n-button size="small" :disabled="loading" @click="show = false">
          {{ $t("common.cancel") }}
        </n-button>
        <n-button
          size="small"
          type="primary"
          :loading="loading"
          :disabled="!valid"
          @click="handleSubmit"
        >
          {{ $t("downloads.editAndRetry") }}
        </n-button>
      </n-flex>
    </template>
  </n-modal>
</template>

<style scoped>
.edit-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 14px;
}
@media (max-width: 520px) {
  .edit-grid {
    grid-template-columns: 1fr;
  }
}
</style>
