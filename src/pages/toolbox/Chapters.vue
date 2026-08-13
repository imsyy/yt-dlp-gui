<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { showErrorDialog, sanitizeFilename } from "@/utils/format";
import { isValidUrl } from "@/utils/validate";
import { useSettingStore } from "@/stores/setting";
import { useStatusStore } from "@/stores/status";
import { useVideoStore } from "@/stores/video";
import { useI18n } from "vue-i18n";
import type { Chapter, ChaptersInfo } from "@/types";

const { t } = useI18n();
const settingStore = useSettingStore();
const statusStore = useStatusStore();
const videoStore = useVideoStore();
const toolUrl = inject<Ref<string>>("toolUrl")!;

const loading = ref(false);
const videoTitle = ref("");
const videoDuration = ref<number | null>(null);
const chapters = ref<Chapter[]>([]);
const exportFormat = ref<"json" | "csv">("json");

const urlValid = computed(() => isValidUrl(toolUrl.value.trim()));

const formatTime = (secs: number): string => {
  if (!Number.isFinite(secs) || secs < 0) return "--:--:--";
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = Math.floor(secs % 60);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${pad(h)}:${pad(m)}:${pad(s)}`;
};

const formatDuration = (start: number, end: number): string => {
  const d = Math.max(0, end - start);
  const m = Math.floor(d / 60);
  const s = Math.floor(d % 60);
  return m > 0 ? `${m}m ${s}s` : `${s}s`;
};

const handleFetch = async () => {
  loading.value = true;
  chapters.value = [];
  videoTitle.value = "";
  videoDuration.value = null;
  try {
    const { cookieFile, cookieBrowser } = await videoStore.getCookieArgs();
    const info = await invoke<ChaptersInfo>("tool_fetch_chapters", {
      url: toolUrl.value.trim(),
      cookieFile,
      cookieBrowser,
      proxy: settingStore.proxy || null,
    });
    videoTitle.value = info.title || "";
    videoDuration.value = info.duration ?? null;
    chapters.value = info.chapters || [];
    if (chapters.value.length === 0) {
      window.$message.warning(t("toolbox.noChaptersFound"));
    }
  } catch (e: unknown) {
    const msg = String(e);
    if (/err_ytdlp_not_installed/.test(msg)) {
      statusStore.showYtdlpSetupModal = true;
    } else if (/sign in|cookies/i.test(msg)) {
      window.$message.warning(t("cookie.verificationDesc"));
      statusStore.showCookieModal = true;
    } else {
      showErrorDialog(msg);
    }
  } finally {
    loading.value = false;
  }
};

const handleCopy = async (chapter: Chapter) => {
  const text = `${formatTime(chapter.start_time)} - ${formatTime(chapter.end_time)}  ${chapter.title}`;
  try {
    await writeText(text);
    window.$message.success(t("toolbox.chapterCopied"));
  } catch {
    window.$message.error(t("clipboard.readFailed"));
  }
};

const exportJson = () =>
  JSON.stringify(
    chapters.value.map((c) => ({
      title: c.title,
      start_time: c.start_time,
      end_time: c.end_time,
      start: formatTime(c.start_time),
      end: formatTime(c.end_time),
    })),
    null,
    2,
  );

const exportCsv = () => {
  const header = `"title","start_time","end_time","start","end"`;
  const rows = chapters.value.map((c) =>
    [
      `"${c.title.replace(/"/g, '""')}"`,
      c.start_time,
      c.end_time,
      `"${formatTime(c.start_time)}"`,
      `"${formatTime(c.end_time)}"`,
    ].join(","),
  );
  return [header, ...rows].join("\r\n");
};

const handleSave = async () => {
  if (chapters.value.length === 0) return;
  const ext = exportFormat.value;
  const defaultName = videoTitle.value
    ? `${sanitizeFilename(videoTitle.value).slice(0, 200)}.chapters.${ext}`
    : `chapters.${ext}`;
  const filePath = await save({
    title: t("toolbox.saveChapters"),
    defaultPath: defaultName,
    filters: [{ name: ext.toUpperCase(), extensions: [ext] }],
  });
  if (!filePath) return;
  try {
    const content = ext === "json" ? exportJson() : exportCsv();
    await invoke("tool_save_text_to_file", { content, filePath });
    window.$message.success(t("toolbox.chaptersSaved"));
  } catch (e: unknown) {
    window.$message.error(t("common.saveFailed", { e }));
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
        {{ $t("common.back") }}
      </n-button>
      <n-text strong style="font-size: 15px">{{ $t("toolbox.chaptersTitle") }}</n-text>
    </n-flex>

    <n-card size="small">
      <n-flex vertical :size="12">
        <n-text depth="3" style="font-size: 13px">
          {{ $t("toolbox.chaptersPageDesc") }}
        </n-text>
        <n-button
          type="primary"
          :loading="loading"
          :disabled="!urlValid || loading"
          @click="handleFetch"
        >
          <template #icon>
            <n-icon><icon-mdi-format-list-numbered /></n-icon>
          </template>
          {{ $t("toolbox.fetchChapters") }}
        </n-button>
      </n-flex>
    </n-card>

    <n-card
      v-if="chapters.length"
      size="small"
      :title="$t('toolbox.chapterCount', { n: chapters.length })"
    >
      <template #header-extra>
        <n-flex align="center" :size="8">
          <n-select
            v-model:value="exportFormat"
            :options="[
              { label: 'JSON', value: 'json' },
              { label: 'CSV', value: 'csv' },
            ]"
            size="small"
            style="width: 100px"
          />
          <n-button size="small" type="primary" secondary @click="handleSave">
            <template #icon>
              <n-icon><icon-mdi-content-save-outline /></n-icon>
            </template>
            {{ $t("common.saveAs") }}
          </n-button>
        </n-flex>
      </template>
      <n-list hoverable bordered>
        <n-list-item v-for="(chapter, i) in chapters" :key="i">
          <n-flex align="center" :size="12" :wrap="false">
            <n-text depth="3" style="font-size: 12px; min-width: 28px; text-align: right">
              {{ i + 1 }}
            </n-text>
            <n-flex vertical :size="2" style="flex: 1; min-width: 0">
              <n-text strong class="chapter-title">
                {{ chapter.title || $t("common.unknown") }}
              </n-text>
              <n-text depth="3" style="font-size: 12px">
                {{ formatTime(chapter.start_time) }} - {{ formatTime(chapter.end_time) }}
                ·
                {{ formatDuration(chapter.start_time, chapter.end_time) }}
              </n-text>
            </n-flex>
            <n-button size="small" quaternary @click="handleCopy(chapter)">
              <template #icon>
                <n-icon><icon-mdi-content-copy /></n-icon>
              </template>
              {{ $t("common.copy") }}
            </n-button>
          </n-flex>
        </n-list-item>
      </n-list>
    </n-card>
  </n-flex>
</template>

<style scoped lang="scss">
.chapter-title {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  font-size: 13px;
}
</style>
