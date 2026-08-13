<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { showErrorDialog, sanitizeFilename } from "@/utils/format";
import { isValidUrl } from "@/utils/validate";
import { useSettingStore } from "@/stores/setting";
import { useStatusStore } from "@/stores/status";
import { useVideoStore } from "@/stores/video";
import { useI18n } from "vue-i18n";
import type { CommentsInfo, VideoComment } from "@/types";
import type { DataTableColumns, DataTableRowKey } from "naive-ui";

const { t } = useI18n();
const settingStore = useSettingStore();
const statusStore = useStatusStore();
const videoStore = useVideoStore();
const toolUrl = inject<Ref<string>>("toolUrl")!;

const loading = ref(false);
const saving = ref(false);
const comments = ref<VideoComment[]>([]);
const videoTitle = ref("");
const totalCount = ref<number | null>(null);

const maxComments = ref(200);
const sortBy = ref<"top" | "new">("top");
const onlyTopLevel = ref(false);
const onlyUploader = ref(false);

const filterText = ref("");
const debouncedFilter = refDebounced(filterText, 300);
const useRegex = ref(false);
const checkedKeys = ref<DataTableRowKey[]>([]);
const exportFormat = ref<"json" | "csv">("json");

const urlValid = computed(() => isValidUrl(toolUrl.value.trim()));

const fieldDefs = computed(
  () =>
    [
      { key: "author", label: t("toolbox.commentAuthor") },
      { key: "text", label: t("toolbox.commentText") },
      { key: "like_count", label: t("toolbox.commentLikes") },
      { key: "timestamp", label: t("toolbox.commentTime") },
      { key: "parent", label: t("toolbox.commentParent") },
      { key: "id", label: t("toolbox.commentId") },
      { key: "author_id", label: t("toolbox.commentAuthorId") },
      { key: "author_is_uploader", label: t("toolbox.commentByUploader") },
    ] as const,
);

type FieldKey =
  | "author"
  | "text"
  | "like_count"
  | "timestamp"
  | "parent"
  | "id"
  | "author_id"
  | "author_is_uploader";
const allFieldKeys: FieldKey[] = [
  "author",
  "text",
  "like_count",
  "timestamp",
  "parent",
  "id",
  "author_id",
  "author_is_uploader",
];
const selectedFields = ref<FieldKey[]>(["author", "text", "like_count", "timestamp"]);

const allFieldsSelected = computed(() => selectedFields.value.length === allFieldKeys.length);
const someFieldsSelected = computed(
  () => selectedFields.value.length > 0 && selectedFields.value.length < allFieldKeys.length,
);
const handleFieldSelectAll = (checked: boolean) => {
  selectedFields.value = checked ? [...allFieldKeys] : [];
};

const filterRegex = computed(() => {
  const text = debouncedFilter.value.trim();
  if (!text) return null;
  if (useRegex.value) {
    try {
      return new RegExp(text, "i");
    } catch {
      return null;
    }
  }
  return new RegExp(text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"), "i");
});

const regexError = computed(() => {
  if (!useRegex.value || !filterText.value.trim()) return "";
  try {
    new RegExp(filterText.value.trim());
    return "";
  } catch (e) {
    return String(e).replace("SyntaxError: ", "");
  }
});

const filteredComments = computed(() => {
  let list = comments.value;
  if (onlyTopLevel.value) {
    list = list.filter((c) => c.parent === "root" || !c.parent);
  }
  if (onlyUploader.value) {
    list = list.filter((c) => c.author_is_uploader);
  }
  const re = filterRegex.value;
  if (re) {
    list = list.filter((c) => re.test(c.text) || re.test(c.author));
  }
  return list;
});

watch([debouncedFilter, onlyTopLevel, onlyUploader], () => {
  checkedKeys.value = [];
});

const formatTimestamp = (ts: number): string => {
  if (!ts) return "";
  const d = new Date(ts * 1000);
  if (isNaN(d.getTime())) return "";
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
};

const columns = computed<DataTableColumns<VideoComment>>(() => [
  { type: "selection" },
  {
    title: t("toolbox.commentAuthor"),
    key: "author",
    width: 180,
    ellipsis: { tooltip: true },
    render: (row) => {
      if (row.author_is_uploader) return `${row.author} ★`;
      return row.author;
    },
  },
  {
    title: t("toolbox.commentText"),
    key: "text",
    minWidth: 240,
    ellipsis: { tooltip: true },
  },
  {
    title: t("toolbox.commentLikes"),
    key: "like_count",
    width: 80,
    sorter: (a, b) => a.like_count - b.like_count,
  },
  {
    title: t("toolbox.commentTime"),
    key: "timestamp",
    width: 140,
    render: (row) => formatTimestamp(row.timestamp),
    sorter: (a, b) => a.timestamp - b.timestamp,
  },
]);

const rowKey = (row: VideoComment) => row.id;

const handleFetch = async () => {
  loading.value = true;
  comments.value = [];
  checkedKeys.value = [];
  filterText.value = "";
  totalCount.value = null;
  try {
    const { cookieFile, cookieBrowser } = await videoStore.getCookieArgs();
    const info = await invoke<CommentsInfo>("tool_fetch_comments", {
      url: toolUrl.value.trim(),
      maxComments: maxComments.value,
      sort: sortBy.value,
      cookieFile,
      cookieBrowser,
      proxy: settingStore.proxy || null,
    });
    videoTitle.value = info.title || "";
    totalCount.value = info.comment_count ?? null;
    comments.value = info.comments || [];
    if (comments.value.length === 0) {
      window.$message.warning(t("toolbox.noCommentsFound"));
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

const buildExportData = () => {
  const keys = selectedFields.value;
  const checkedSet = new Set(checkedKeys.value);
  const source =
    checkedKeys.value.length > 0
      ? filteredComments.value.filter((c) => checkedSet.has(c.id))
      : filteredComments.value;
  return source.map((c) => {
    const obj: Record<string, unknown> = {};
    for (const key of keys) {
      if (key === "timestamp") {
        obj.timestamp = c.timestamp;
        obj.time = formatTimestamp(c.timestamp);
      } else {
        obj[key] = c[key];
      }
    }
    return obj;
  });
};

const exportJson = (data: Record<string, unknown>[]) => JSON.stringify(data, null, 2);

const exportCsv = (data: Record<string, unknown>[]) => {
  if (data.length === 0) return "";
  const keys = Object.keys(data[0]);
  const header = keys.map((k) => `"${k}"`).join(",");
  const rows = data.map((row) =>
    keys
      .map((k) => {
        const val = String(row[k] ?? "");
        return `"${val.replace(/"/g, '""')}"`;
      })
      .join(","),
  );
  return [header, ...rows].join("\r\n");
};

const exportCount = computed(() => {
  if (checkedKeys.value.length > 0) return checkedKeys.value.length;
  return filteredComments.value.length;
});

const handleSave = async () => {
  if (selectedFields.value.length === 0) {
    window.$message.warning(t("toolbox.selectAtLeastOneField"));
    return;
  }
  const ext = exportFormat.value;
  const defaultName = videoTitle.value
    ? `${sanitizeFilename(videoTitle.value).slice(0, 200)}.comments.${ext}`
    : `comments.${ext}`;
  const filePath = await save({
    title: t("toolbox.saveComments"),
    defaultPath: defaultName,
    filters: [{ name: ext.toUpperCase(), extensions: [ext] }],
  });
  if (!filePath) return;

  saving.value = true;
  try {
    const data = buildExportData();
    const content = ext === "json" ? exportJson(data) : exportCsv(data);
    await invoke("tool_save_text_to_file", { content, filePath });
    window.$message.success(t("toolbox.commentsSaved"));
  } catch (e: unknown) {
    window.$message.error(t("common.saveFailed", { e }));
  } finally {
    saving.value = false;
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
      <n-text strong style="font-size: 15px">{{ $t("toolbox.commentsTitle") }}</n-text>
    </n-flex>

    <n-card size="small">
      <n-flex vertical :size="12">
        <n-text depth="3" style="font-size: 13px">
          {{ $t("toolbox.commentsPageDesc") }}
        </n-text>
        <n-flex align="center" :size="8" :wrap="false">
          <n-flex align="center" :size="6">
            <n-text style="font-size: 13px">{{ $t("toolbox.commentMaxCount") }}</n-text>
            <n-input-number
              v-model:value="maxComments"
              :min="10"
              :max="5000"
              :step="50"
              size="small"
              style="width: 110px"
            />
          </n-flex>
          <n-flex align="center" :size="6">
            <n-text style="font-size: 13px">{{ $t("toolbox.commentSort") }}</n-text>
            <n-radio-group v-model:value="sortBy" size="small">
              <n-radio-button value="top">{{ $t("toolbox.commentSortTop") }}</n-radio-button>
              <n-radio-button value="new">{{ $t("toolbox.commentSortNew") }}</n-radio-button>
            </n-radio-group>
          </n-flex>
        </n-flex>
        <n-button
          type="primary"
          :loading="loading"
          :disabled="!urlValid || loading"
          @click="handleFetch"
        >
          <template #icon>
            <n-icon><icon-mdi-comment-text-multiple-outline /></n-icon>
          </template>
          {{ $t("toolbox.fetchComments") }}
        </n-button>
      </n-flex>
    </n-card>

    <n-card
      v-if="comments.length"
      size="small"
      :title="$t('toolbox.commentTotal', { n: filteredComments.length, total: comments.length })"
    >
      <template #header-extra>
        <n-flex align="center" :size="8">
          <n-popover trigger="click" placement="bottom-end">
            <template #trigger>
              <n-button size="small" secondary>
                <template #icon>
                  <n-icon><icon-mdi-filter-outline /></n-icon>
                </template>
                {{ $t("toolbox.exportFields") }}
              </n-button>
            </template>
            <n-flex vertical :size="8" style="min-width: 160px">
              <n-checkbox
                :checked="allFieldsSelected"
                :indeterminate="someFieldsSelected"
                @update:checked="handleFieldSelectAll"
              >
                {{ $t("common.selectAll") }}
              </n-checkbox>
              <n-checkbox-group v-model:value="selectedFields">
                <n-flex vertical :size="4">
                  <n-checkbox v-for="f in fieldDefs" :key="f.key" :value="f.key">
                    {{ f.label }}
                  </n-checkbox>
                </n-flex>
              </n-checkbox-group>
            </n-flex>
          </n-popover>
          <n-select
            v-model:value="exportFormat"
            :options="[
              { label: 'JSON', value: 'json' },
              { label: 'CSV', value: 'csv' },
            ]"
            size="small"
            style="width: 100px"
          />
          <n-button
            size="small"
            type="primary"
            secondary
            :loading="saving"
            :disabled="exportCount === 0"
            @click="handleSave"
          >
            <template #icon>
              <n-icon><icon-mdi-content-save-outline /></n-icon>
            </template>
            {{ $t("toolbox.saveAsCount", { n: exportCount }) }}
          </n-button>
        </n-flex>
      </template>

      <n-flex vertical :size="8">
        <n-flex align="center" :size="8">
          <n-input
            v-model:value="filterText"
            :placeholder="$t('toolbox.commentSearchPlaceholder')"
            size="small"
            clearable
            style="flex: 1"
            :status="regexError ? 'error' : undefined"
          >
            <template #prefix>
              <n-icon><icon-mdi-magnify /></n-icon>
            </template>
            <template #suffix>
              <n-tooltip :disabled="!regexError">
                <template #trigger>
                  <n-button
                    text
                    :type="useRegex ? 'primary' : 'default'"
                    @click="useRegex = !useRegex"
                  >
                    <n-text :depth="useRegex ? undefined : 3" style="font-family: monospace">
                      .*
                    </n-text>
                  </n-button>
                </template>
                {{ regexError }}
              </n-tooltip>
            </template>
          </n-input>
          <n-checkbox v-model:checked="onlyTopLevel">
            {{ $t("toolbox.commentOnlyTopLevel") }}
          </n-checkbox>
          <n-checkbox v-model:checked="onlyUploader">
            {{ $t("toolbox.commentOnlyUploader") }}
          </n-checkbox>
        </n-flex>

        <n-data-table
          v-model:checked-row-keys="checkedKeys"
          :columns="columns"
          :data="filteredComments"
          :row-key="rowKey"
          :pagination="{ pageSize: 50 }"
          size="small"
          :max-height="500"
          :scroll-x="700"
        />
      </n-flex>
    </n-card>
  </n-flex>
</template>
