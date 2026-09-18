<script setup lang="ts">
import { open } from "@tauri-apps/plugin-dialog";
import { useSettingStore } from "@/stores/setting";
import { useI18n } from "vue-i18n";

interface Props {
  plain?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  plain: false,
});

const { t } = useI18n();
const settingStore = useSettingStore();

/** 打开文件夹选择对话框 */
const handleSelectDir = async () => {
  const selected = await open({
    directory: true,
    multiple: false,
    title: t("downloadDir.selectDir"),
  });
  if (selected) {
    settingStore.downloadDir = selected as string;
  }
};
</script>

<template>
  <template v-if="props.plain">
    <n-flex align="center" :size="8" :wrap="false">
      <n-text depth="3" class="option-label">{{ $t("downloadDir.title") }}</n-text>
      <n-input
        :value="settingStore.downloadDir"
        :placeholder="$t('downloadDir.notSet')"
        size="small"
        readonly
        style="flex: 1"
      />
      <n-button size="small" @click="handleSelectDir">
        <template #icon>
          <n-icon>
            <icon-mdi-folder-open-outline />
          </n-icon>
        </template>
        {{ $t("common.select") }}
      </n-button>
    </n-flex>
  </template>
  <n-card v-else :title="$t('downloadDir.title')" size="small">
    <n-flex align="center" :size="8">
      <n-input
        :value="settingStore.downloadDir"
        :placeholder="$t('downloadDir.notSet')"
        size="small"
        readonly
        style="flex: 1"
      />
      <n-button size="small" @click="handleSelectDir">
        <template #icon>
          <n-icon>
            <icon-mdi-folder-open-outline />
          </n-icon>
        </template>
        {{ $t("common.select") }}
      </n-button>
    </n-flex>
  </n-card>
</template>

<style scoped lang="scss">
.option-label {
  min-width: 56px;
  flex-shrink: 0;
  font-size: 13px;
  white-space: nowrap;
}
</style>
