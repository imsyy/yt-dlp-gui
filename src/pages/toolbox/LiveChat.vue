<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { formatError } from "@/utils/format";
import { isValidUrl } from "@/utils/validate";
import { useSettingStore } from "@/stores/setting";
import { useStatusStore } from "@/stores/status";
import { useVideoStore } from "@/stores/video";
import { useI18n } from "vue-i18n";
import type { LiveChatMessage } from "@/types";
import type { DataTableColumns, DataTableRowKey } from "naive-ui";

const { t } = useI18n();
const settingStore = useSettingStore();
const statusStore = useStatusStore();
const videoStore = useVideoStore();
const toolUrl = inject<Ref<string>>("toolUrl")!;

const loading = ref(false);
const saving = ref(false);
const messages = ref<LiveChatMessage[]>([]);
const checkedKeys = ref<DataTableRowKey[]>([]);
const filterText = ref("");
const debouncedFilter = refDebounced(filterText, 300);
const useRegex = ref(false);

const urlValid = computed(() => isValidUrl(toolUrl.value.trim()));

const fieldDefs = computed(() => [
  { key: "time", label: t("toolbox.time") },
  { key: "author", label: t("toolbox.sender") },
  { key: "message", label: t("toolbox.message") },
  { key: "msg_type", label: t("toolbox.type") },
  { key: "amount", label: t("toolbox.tipAmount") },
  { key: "channel_id", label: t("toolbox.channelId") },
  { key: "timestamp_usec", label: t("toolbox.timestampUs") },
] as const);

type FieldKey = "time" | "author" | "message" | "msg_type" | "amount" | "channel_id" | "timestamp_usec";
const allFieldKeys: FieldKey[] = ["time", "author", "message", "msg_type", "amount", "channel_id", "timestamp_usec"];
const selectedFields = ref<FieldKey[]>([...allFieldKeys]);

const exportFormat = ref<"json" | "csv">("json");

const allFieldsSelected = computed(() => selectedFields.value.length === allFieldKeys.length);
const someFieldsSelected = computed(
  () => selectedFields.value.length > 0 && selectedFields.value.length < allFieldKeys.length,
);
const handleFieldSelectAll = (checked: boolean) => {
  selectedFields.value = checked ? [...allFieldKeys] : [];
};

const typeLabels = computed<Record<string, string>>(() => ({
  text: t("toolbox.typeNormal"),
  paid: t("toolbox.typePaid"),
  membership: t("toolbox.typeMembership"),
}));

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

const filteredMessages = computed(() => {
  const re = filterRegex.value;
  if (!re) return messages.value;
  return messages.value.filter(
    (m) => re.test(m.message) || re.test(m.author),
  );
});

watch(debouncedFilter, () => {
  checkedKeys.value = [];
});

const columns = computed<DataTableColumns<LiveChatMessage>>(() => [
  { type: "selection" },
  { title: t("toolbox.time"), key: "time", width: 100 },
  { title: t("toolbox.sender"), key: "author", width: 160, ellipsis: { tooltip: true } },
  {
    title: t("toolbox.message"),
    key: "message",
    minWidth: 200,
    ellipsis: { tooltip: true },
  },
  {
    title: t("toolbox.type"),
    key: "msg_type",
    width: 80,
    render: (row) => typeLabels.value[row.msg_type] || row.msg_type,
  },
  { title: t("toolbox.amount"), key: "amount", width: 100 },
]);

const rowKey = (row: LiveChatMessage) => row.idx;

/** 获取弹幕数据 */
const handleFetch = async () => {
  loading.value = true;
  messages.value = [];
  checkedKeys.value = [];
  filterText.value = "";
  try {
    const cookieFile = await videoStore.getCookieFile();
    const result = await invoke<LiveChatMessage[]>("tool_fetch_live_chat", {
      url: toolUrl.value.trim(),
      cookieFile,
      proxy: settingStore.proxy || null,
    });
    messages.value = result;
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

/** 构建导出数据：有选中导出选中行，否则导出筛选后的全部行 */
const buildExportData = () => {
  const keys = selectedFields.value;
  const checkedSet = new Set(checkedKeys.value);
  const source =
    checkedKeys.value.length > 0
      ? filteredMessages.value.filter((m) => checkedSet.has(m.idx))
      : filteredMessages.value;
  return source.map((msg) => {
    const obj: Record<string, unknown> = {};
    for (const key of keys) {
      obj[key] = msg[key];
    }
    return obj;
  });
};

const exportJson = (data: Record<string, unknown>[]) => {
  return JSON.stringify(data, null, 2);
};

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

/** 导出条数提示 */
const exportCount = computed(() => {
  if (checkedKeys.value.length > 0) return checkedKeys.value.length;
  return filteredMessages.value.length;
});

/** 另存为文件 */
const handleSave = async () => {
  if (selectedFields.value.length === 0) {
    window.$message.warning(t("toolbox.selectAtLeastOneField"));
    return;
  }

  const ext = exportFormat.value;
  const filePath = await save({
    title: t("toolbox.saveChatData"),
    defaultPath: `live_chat.${ext}`,
    filters: [{ name: ext.toUpperCase(), extensions: [ext] }],
  });
  if (!filePath) return;

  saving.value = true;
  try {
    const data = buildExportData();
    const content = ext === "json" ? exportJson(data) : exportCsv(data);
    await invoke("tool_save_text_to_file", { content, filePath });
    window.$message.success(t("toolbox.chatDataSaved"));
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
        {{ $t('common.back') }}
      </n-button>
      <n-text strong style="font-size: 15px">{{ $t('toolbox.livechatTitle') }}</n-text>
    </n-flex>

    <n-card size="small">
      <n-flex vertical :size="12">
        <n-text depth="3" style="font-size: 13px">
          {{ $t('toolbox.livechatPageDesc') }}
        </n-text>
        <n-button
          type="primary"
          :loading="loading"
          :disabled="!urlValid || loading"
          @click="handleFetch"
        >
          <template #icon>
            <n-icon><icon-mdi-message-text-outline /></n-icon>
          </template>
          {{ $t('toolbox.fetchChat') }}
        </n-button>
      </n-flex>
    </n-card>

    <n-card v-if="messages.length" size="small" :title="$t('toolbox.chatCount', { n: messages.length })">
      <template #header-extra>
        <n-flex align="center" :size="8">
          <n-popover trigger="click" placement="bottom-end">
            <template #trigger>
              <n-button size="small" secondary>
                <template #icon>
                  <n-icon><icon-mdi-filter-outline /></n-icon>
                </template>
                {{ $t('toolbox.exportFields') }}
              </n-button>
            </template>
            <n-flex vertical :size="8" style="min-width: 140px">
              <n-checkbox
                :checked="allFieldsSelected"
                :indeterminate="someFieldsSelected"
                @update:checked="handleFieldSelectAll"
              >
                {{ $t('common.selectAll') }}
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
            style="width: 90px"
          />
          <n-button
            size="small"
            :loading="saving"
            :disabled="selectedFields.length === 0 || saving"
            @click="handleSave"
          >
            <template #icon>
              <n-icon><icon-mdi-content-save-outline /></n-icon>
            </template>
            {{ $t('toolbox.saveAsCount', { n: exportCount }) }}
          </n-button>
        </n-flex>
      </template>

      <n-flex vertical :size="10">
        <n-flex align="center" :size="8">
          <n-input
            v-model:value="filterText"
            :placeholder="$t('toolbox.searchPlaceholder')"
            size="small"
            clearable
            style="flex: 1"
            :status="regexError ? 'error' : undefined"
          >
            <template #prefix>
              <n-icon><icon-mdi-magnify /></n-icon>
            </template>
          </n-input>
          <n-tooltip>
            <template #trigger>
              <n-button
                size="small"
                :type="useRegex ? 'primary' : 'default'"
                :secondary="useRegex"
                :quaternary="!useRegex"
                @click="useRegex = !useRegex"
                style="font-family: monospace; font-weight: bold; width: 32px"
              >
                .*
              </n-button>
            </template>
            {{ useRegex ? $t('toolbox.regexEnabled') : $t('toolbox.enableRegex') }}
          </n-tooltip>
        </n-flex>
        <n-text
          v-if="filterText && !regexError"
          depth="3"
          style="font-size: 12px"
        >
          {{ $t('toolbox.matchCount', { matched: filteredMessages.length, total: messages.length }) }}
        </n-text>
        <n-text v-if="regexError" type="error" style="font-size: 12px">
          {{ regexError }}
        </n-text>

        <n-data-table
          :columns="columns"
          :data="filteredMessages"
          :row-key="rowKey"
          virtual-scroll
          :max-height="480"
          size="small"
          bordered
          @update:checked-row-keys="(keys: DataTableRowKey[]) => (checkedKeys = keys)"
        />
      </n-flex>
    </n-card>
  </n-flex>
</template>
