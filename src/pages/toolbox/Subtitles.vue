<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { formatError } from "@/utils/format";
import { isValidUrl } from "@/utils/validate";
import { mergeBilingualSrt, mergeBilingualVtt } from "@/utils/subtitle";
import { useSettingStore } from "@/stores/setting";
import { useStatusStore } from "@/stores/status";
import { useVideoStore } from "@/stores/video";
import { useI18n } from "vue-i18n";
import type { SubtitleInfo, SubtitleTrack } from "@/types";
import type { SelectOption } from "naive-ui";

const { t } = useI18n();
const settingStore = useSettingStore();
const statusStore = useStatusStore();
const videoStore = useVideoStore();
const toolUrl = inject<Ref<string>>("toolUrl")!;

const loading = ref(false);
const savingKey = ref<string | null>(null);
const savingBilingual = ref(false);
const videoTitle = ref("");
const includeAutoSubs = ref(false);
const exportFormat = ref("srt");

interface SubItem {
  lang: string;
  name: string;
  isAuto: boolean;
  tracks: SubtitleTrack[];
}

const subtitleList = ref<SubItem[]>([]);

const primaryLang = ref<string | null>(null);
const secondaryLang = ref<string | null>(null);

const urlValid = computed(() => isValidUrl(toolUrl.value.trim()));

const formatOptions = [
  { label: "SRT (.srt)", value: "srt" },
  { label: "VTT (.vtt)", value: "vtt" },
  { label: "ASS (.ass)", value: "ass" },
  { label: "LRC (.lrc)", value: "lrc" },
];

/** 从字幕轨道列表中找到指定格式（或最接近的格式）的 URL */
const findTrackUrl = (tracks: SubtitleTrack[], format: string): string | null => {
  const exact = tracks.find((t) => t.ext === format);
  if (exact) return exact.url;
  const vtt = tracks.find((t) => t.ext === "vtt");
  if (vtt) return vtt.url;
  const srt = tracks.find((t) => t.ext === "srt");
  if (srt) return srt.url;
  const json3 = tracks.find((t) => t.ext === "json3");
  if (json3) return json3.url;
  return tracks.length > 0 ? tracks[0].url : null;
};

/** 显示的字幕列表（根据是否包含自动生成过滤） */
const displayList = computed(() => {
  if (includeAutoSubs.value) return subtitleList.value;
  return subtitleList.value.filter((s) => !s.isAuto);
});

/** 语言选项（用于双语合成选择器） */
const langOptions = computed<SelectOption[]>(() =>
  displayList.value.map((s) => ({
    label: `${s.name || s.lang}${s.isAuto ? ` (${t("toolbox.auto")})` : ""}`,
    value: `${s.lang}|${s.isAuto ? "auto" : "manual"}`,
  })),
);

/** 获取字幕列表 */
const handleFetch = async () => {
  loading.value = true;
  subtitleList.value = [];
  videoTitle.value = "";
  primaryLang.value = null;
  secondaryLang.value = null;
  try {
    const cookieFile = await videoStore.getCookieFile();
    const info = await invoke<SubtitleInfo>("tool_fetch_subtitles", {
      url: toolUrl.value.trim(),
      cookieFile,
      proxy: settingStore.proxy || null,
    });
    videoTitle.value = info.title || "";

    const items: SubItem[] = [];

    if (info.subtitles) {
      for (const [lang, tracks] of Object.entries(info.subtitles)) {
        if (tracks && tracks.length > 0) {
          const name = tracks[0]?.name || lang;
          items.push({ lang, name, isAuto: false, tracks });
        }
      }
    }

    if (info.automatic_captions) {
      for (const [lang, tracks] of Object.entries(info.automatic_captions)) {
        if (tracks && tracks.length > 0) {
          const name = tracks[0]?.name || lang;
          if (!items.some((it) => it.lang === lang && !it.isAuto)) {
            items.push({ lang, name, isAuto: true, tracks });
          }
        }
      }
    }

    subtitleList.value = items;

    if (items.length === 0) {
      window.$message.warning(t("toolbox.noSubtitlesFound"));
    }
  } catch (e: unknown) {
    const msg = String(e);
    if (/err_ytdlp_not_installed/.test(msg)) {
      statusStore.showYtdlpSetupModal = true;
    } else if (/sign in|cookies/i.test(msg)) {
      statusStore.showCookieModal = true;
    } else {
      window.$message.error(formatError(msg));
    }
  } finally {
    loading.value = false;
  }
};

/** 另存为单个字幕 */
const handleSave = async (item: SubItem) => {
  const ext = exportFormat.value;
  const url = findTrackUrl(item.tracks, ext);
  if (!url) {
    window.$message.warning(t("toolbox.noSubtitleUrl"));
    return;
  }

  const defaultName = videoTitle.value
    ? `${videoTitle.value.slice(0, 200)}.${item.lang}.${ext}`
    : `subtitle.${item.lang}.${ext}`;

  const filePath = await save({
    title: t("toolbox.saveSubtitleFile"),
    defaultPath: defaultName,
    filters: [{ name: t("toolbox.subtitleFiles"), extensions: [ext] }],
  });
  if (!filePath) return;

  const key = `${item.lang}-${item.isAuto}`;
  savingKey.value = key;
  try {
    await invoke("tool_save_subtitle", {
      url,
      filePath,
      proxy: settingStore.proxy || null,
    });
    window.$message.success(t("toolbox.subtitleSaved"));
  } catch (e: unknown) {
    window.$message.error(t("common.saveFailed", { e }));
  } finally {
    savingKey.value = null;
  }
};

/** 解析语言选择器的值 */
const findSubItem = (val: string): SubItem | undefined => {
  const [lang, type] = val.split("|");
  const isAuto = type === "auto";
  return subtitleList.value.find((s) => s.lang === lang && s.isAuto === isAuto);
};

/** 合成双语字幕并另存为 */
const handleSaveBilingual = async () => {
  if (!primaryLang.value || !secondaryLang.value) {
    window.$message.warning(t("toolbox.selectBothLangs"));
    return;
  }
  if (primaryLang.value === secondaryLang.value) {
    window.$message.warning(t("toolbox.langsCantBeSame"));
    return;
  }

  // 双语合成只支持 srt 和 vtt
  const mergeFormat = exportFormat.value === "vtt" ? "vtt" : "srt";

  const primary = findSubItem(primaryLang.value);
  const secondary = findSubItem(secondaryLang.value);
  if (!primary || !secondary) {
    window.$message.warning(t("toolbox.cantFindSubtitles"));
    return;
  }

  const primaryUrl = findTrackUrl(primary.tracks, mergeFormat);
  const secondaryUrl = findTrackUrl(secondary.tracks, mergeFormat);
  if (!primaryUrl || !secondaryUrl) {
    window.$message.warning(t("toolbox.noSubtitleUrl"));
    return;
  }

  const defaultName = videoTitle.value
    ? `${videoTitle.value.slice(0, 200)}.${primary.lang}-${secondary.lang}.${mergeFormat}`
    : `subtitle.${primary.lang}-${secondary.lang}.${mergeFormat}`;

  const filePath = await save({
    title: t("toolbox.saveBilingualSubtitle"),
    defaultPath: defaultName,
    filters: [{ name: t("toolbox.subtitleFiles"), extensions: [mergeFormat] }],
  });
  if (!filePath) return;

  savingBilingual.value = true;
  try {
    const proxy = settingStore.proxy || null;
    const [primaryText, secondaryText] = await Promise.all([
      invoke<string>("tool_download_text", { url: primaryUrl, proxy }),
      invoke<string>("tool_download_text", { url: secondaryUrl, proxy }),
    ]);

    const merged =
      mergeFormat === "vtt"
        ? mergeBilingualVtt(primaryText, secondaryText)
        : mergeBilingualSrt(primaryText, secondaryText);

    await invoke("tool_save_text_to_file", { content: merged, filePath });
    window.$message.success(t("toolbox.bilingualSaved"));
  } catch (e: unknown) {
    window.$message.error(t("common.saveFailed", { e }));
  } finally {
    savingBilingual.value = false;
  }
};
</script>

<template>
  <n-flex vertical :size="12">
    <n-flex align="center" :size="8">
      <n-button strong secondary size="small" @click="$router.back()">
        <template #icon>
          <n-icon><icon-mdi-arrow-left /></n-icon>
        </template>
        {{ $t('common.back') }}
      </n-button>
      <n-text strong style="font-size: 15px">{{ $t('toolbox.subtitlesTitle') }}</n-text>
    </n-flex>

    <n-card size="small">
      <n-flex vertical :size="12">
        <n-text depth="3" style="font-size: 13px">
          {{ $t('toolbox.subtitlesPageDesc') }}
        </n-text>
        <n-button
          type="primary"
          :loading="loading"
          :disabled="!urlValid || loading"
          @click="handleFetch"
        >
          <template #icon>
            <n-icon><icon-mdi-subtitles-outline /></n-icon>
          </template>
          {{ $t('toolbox.fetchSubtitles') }}
        </n-button>
      </n-flex>
    </n-card>

    <n-card v-if="subtitleList.length" size="small" :title="$t('toolbox.subtitleCount', { n: displayList.length })">
      <template #header-extra>
        <n-flex align="center" :size="8">
          <n-checkbox v-model:checked="includeAutoSubs" size="small">{{ $t('toolbox.includeAutoGenerated') }}</n-checkbox>
          <n-select
            v-model:value="exportFormat"
            :options="formatOptions"
            size="small"
            style="width: 140px"
          />
        </n-flex>
      </template>
      <n-list hoverable clickable bordered>
        <n-list-item v-for="item in displayList" :key="`${item.lang}-${item.isAuto}`">
          <n-flex align="center" justify="space-between">
            <n-flex align="center" :size="6">
              <n-text strong style="font-size: 13px">{{ item.name || item.lang }}</n-text>
              <n-tag v-if="item.isAuto" size="small" type="info" :bordered="false">{{ $t('toolbox.auto') }}</n-tag>
              <n-text depth="3" style="font-size: 12px">{{ item.lang }}</n-text>
            </n-flex>
            <n-button
              size="small"
              :loading="savingKey === `${item.lang}-${item.isAuto}`"
              @click="handleSave(item)"
            >
              <template #icon>
                <n-icon><icon-mdi-content-save-outline /></n-icon>
              </template>
              {{ $t('common.saveAs') }}
            </n-button>
          </n-flex>
        </n-list-item>
      </n-list>
    </n-card>

    <n-card v-if="subtitleList.length >= 2" size="small" :title="$t('toolbox.bilingualMerge')">
      <n-flex vertical :size="10">
        <n-text depth="3" style="font-size: 13px">
          {{ $t('toolbox.bilingualDesc') }}
        </n-text>
        <n-flex align="center" :size="8">
          <n-select
            v-model:value="primaryLang"
            :options="langOptions"
            :placeholder="$t('toolbox.primaryLang')"
            size="small"
            clearable
            style="flex: 1"
          />
          <n-select
            v-model:value="secondaryLang"
            :options="langOptions"
            :placeholder="$t('toolbox.secondaryLang')"
            size="small"
            clearable
            style="flex: 1"
          />
        </n-flex>
        <n-button
          type="primary"
          secondary
          :loading="savingBilingual"
          :disabled="!primaryLang || !secondaryLang || savingBilingual"
          @click="handleSaveBilingual"
        >
          <template #icon>
            <n-icon><icon-mdi-content-save-outline /></n-icon>
          </template>
          {{ $t('toolbox.saveBilingual') }}
        </n-button>
      </n-flex>
    </n-card>
  </n-flex>
</template>
