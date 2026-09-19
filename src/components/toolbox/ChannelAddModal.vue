<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { useSettingStore } from "@/stores/setting";
import { useVideoStore } from "@/stores/video";
import { showErrorDialog } from "@/utils/format";
import { isValidUrl } from "@/utils/validate";
import { useI18n } from "vue-i18n";
import type { ChannelRecord } from "@/types";

const show = defineModel<boolean>("show", { required: true });
const emit = defineEmits<{
  (e: "success", channel: ChannelRecord): void;
}>();

const { t } = useI18n();
const settingStore = useSettingStore();
const videoStore = useVideoStore();

const channelUrl = ref("");
const loading = ref(false);

const handleClose = () => {
  if (!loading.value) {
    show.value = false;
  }
};

const handleReset = () => {
  channelUrl.value = "";
};

const handleSubmit = async () => {
  const trimmed = channelUrl.value.trim();
  if (!trimmed) return;
  if (!isValidUrl(trimmed)) {
    window.$message.warning(t("clipboard.invalidUrl"));
    return;
  }
  loading.value = true;
  try {
    const { cookieFile, cookieBrowser } = await videoStore.getCookieArgs();
    const record = await invoke<ChannelRecord>("channel_add", {
      url: trimmed,
      cookieFile,
      cookieBrowser,
      proxy: settingStore.proxy || null,
    });
    window.$message.success(t("channelArchive.addSuccess", { name: record.title }));
    show.value = false;
    emit("success", record);
  } catch (err: unknown) {
    showErrorDialog(String(err));
  } finally {
    loading.value = false;
  }
};
</script>

<template>
  <n-modal
    v-model:show="show"
    preset="card"
    :title="$t('channelArchive.addChannel')"
    size="small"
    :bordered="false"
    content-scrollable
    :segmented="{ action: 'soft' }"
    :style="{ width: '500px', maxHeight: '80vh' }"
    :closable="!loading"
    :mask-closable="!loading"
    @after-leave="handleReset"
  >
    <n-flex vertical :size="14">
      <n-text depth="3" class="desc-text">
        {{ $t("channelArchive.desc") }}
      </n-text>

      <ToolUrlInput
        v-model="channelUrl"
        size="small"
        :placeholder="$t('channelArchive.channelUrlPlaceholder')"
        :disabled="loading"
        @submit="handleSubmit"
      />
    </n-flex>

    <template #action>
      <n-flex justify="end" :size="8">
        <n-button size="small" :disabled="loading" @click="handleClose">
          {{ $t("common.cancel") }}
        </n-button>
        <n-button
          size="small"
          type="primary"
          :loading="loading"
          :disabled="!channelUrl.trim()"
          @click="handleSubmit"
        >
          {{ $t("channelArchive.fetchChannelInfo") }}
        </n-button>
      </n-flex>
    </template>
  </n-modal>
</template>

<style scoped lang="scss">
.desc-text {
  font-size: 13px;
  line-height: 1.5;
}
</style>
