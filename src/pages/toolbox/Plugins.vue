<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { showErrorDialog, formatFileSize } from "@/utils/format";
import { goToolList } from "@/utils/toolbox";
import { useI18n } from "vue-i18n";

/** 后端 list_plugins 返回的插件项 */
interface PluginItem {
  fileName: string;
  name: string;
  sizeBytes: number;
  modifiedAt: number;
  pluginTypes: string[];
}

const { t } = useI18n();
const router = useRouter();

const plugins = ref<PluginItem[]>([]);
const loading = ref(false);
const importing = ref(false);
const removingFile = ref<string | null>(null);

const goBack = () => goToolList(router);

/** 拉取已导入的插件包列表 */
const refresh = async (): Promise<void> => {
  loading.value = true;
  try {
    plugins.value = await invoke<PluginItem[]>("list_plugins");
  } catch (error: unknown) {
    showErrorDialog(String(error));
  } finally {
    loading.value = false;
  }
};

/** 从本地 zip 导入插件包 */
const handleImport = async (): Promise<void> => {
  const selected = await open({
    multiple: false,
    title: t("plugins.pickFile"),
    filters: [
      { name: t("plugins.zipFiles"), extensions: ["zip"] },
      { name: t("plugins.allFiles"), extensions: ["*"] },
    ],
  });
  if (typeof selected !== "string") return;

  importing.value = true;
  try {
    const installed = await invoke<PluginItem>("import_plugin", { sourcePath: selected });
    await refresh();
    window.$message.success(t("plugins.importSuccess", { name: installed.name }));
  } catch (error: unknown) {
    showErrorDialog(String(error));
  } finally {
    importing.value = false;
  }
};

/** 移除插件包 */
const handleRemove = (target: PluginItem): void => {
  window.$dialog.warning({
    title: t("plugins.remove"),
    content: t("plugins.removeConfirm", { name: target.name }),
    positiveText: t("plugins.remove"),
    negativeText: t("common.cancel"),
    onPositiveClick: async () => {
      removingFile.value = target.fileName;
      try {
        await invoke("remove_plugin", { fileName: target.fileName });
        await refresh();
        window.$message.success(t("plugins.removeSuccess", { name: target.name }));
      } catch (error: unknown) {
        showErrorDialog(String(error));
      } finally {
        removingFile.value = null;
      }
    },
  });
};

/** 打开插件目录 */
const handleOpenDir = async (): Promise<void> => {
  try {
    await invoke("open_plugin_dir");
  } catch (error: unknown) {
    showErrorDialog(String(error));
  }
};

onMounted(refresh);
</script>

<template>
  <n-flex vertical :size="12">
    <n-flex align="center" justify="space-between" :wrap="false">
      <n-flex align="center" :size="8">
        <n-button strong secondary size="small" @click="goBack">
          <template #icon>
            <n-icon><icon-mdi-arrow-left /></n-icon>
          </template>
          {{ $t("common.back") }}
        </n-button>
        <n-text strong style="font-size: 15px">{{ $t("plugins.title") }}</n-text>
      </n-flex>
      <n-flex align="center" :size="8">
        <n-button type="primary" size="small" :loading="importing" @click="handleImport">
          <template #icon>
            <n-icon><icon-mdi-folder-open-outline /></n-icon>
          </template>
          {{ $t("plugins.importFromFile") }}
        </n-button>
        <n-button strong secondary size="small" @click="handleOpenDir">
          <template #icon>
            <n-icon><icon-mdi-folder-outline /></n-icon>
          </template>
          {{ $t("plugins.openFolder") }}
        </n-button>
      </n-flex>
    </n-flex>

    <n-alert type="warning" :bordered="false">
      {{ $t("plugins.trustWarning") }}
    </n-alert>

    <n-empty
      v-if="!loading && plugins.length === 0"
      size="small"
      :description="$t('plugins.empty')"
    />

    <n-card v-for="plugin in plugins" :key="plugin.fileName" size="small">
      <n-flex align="center" :size="12" :wrap="false">
        <n-flex vertical :size="2" style="flex: 1; min-width: 0">
          <n-flex align="center" :size="8">
            <n-text strong>{{ plugin.name }}</n-text>
            <n-tag
              v-for="itemType in plugin.pluginTypes"
              :key="itemType"
              size="small"
              round
              :bordered="false"
              type="info"
            >
              {{
                itemType === "extractor"
                  ? $t("plugins.typeExtractor")
                  : itemType === "postprocessor"
                    ? $t("plugins.typePostprocessor")
                    : itemType
              }}
            </n-tag>
          </n-flex>
          <n-text depth="3" style="font-size: 12px">
            {{ formatFileSize(plugin.sizeBytes) }} · {{ plugin.fileName }}
          </n-text>
        </n-flex>
        <n-button
          size="small"
          type="error"
          secondary
          :loading="removingFile === plugin.fileName"
          @click="handleRemove(plugin)"
        >
          <template #icon>
            <n-icon><icon-mdi-delete-outline /></n-icon>
          </template>
          {{ $t("plugins.remove") }}
        </n-button>
      </n-flex>
    </n-card>
  </n-flex>
</template>
