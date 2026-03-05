<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useSettingStore } from "@/stores/setting";
import type { YtdlpStatus, DenoStatus, DownloadProgress } from "@/types";

const settingStore = useSettingStore();

// 平台
const platform = ref("");
const platformLabel = computed(() => {
  const map: Record<string, string> = {
    windows: "Windows",
    macos: "macOS",
    linux: "Linux",
  };
  return map[platform.value] || platform.value;
});

// 主题选项
const themeModeOptions = [
  { label: "跟随系统", value: "auto" },
  { label: "亮色", value: "light" },
  { label: "暗色", value: "dark" },
];

// ========== yt-dlp ==========
const ytdlpStatus = ref<YtdlpStatus | null>(null);
const ytdlpChecking = ref(true);
const ytdlpDownloading = ref(false);
const ytdlpDownloadPercent = ref(0);
const ytdlpUpdating = ref(false);

async function checkYtdlpStatus() {
  ytdlpChecking.value = true;
  try {
    ytdlpStatus.value = await invoke<YtdlpStatus>("get_ytdlp_status");
  } catch (e) {
    console.error("Failed to check yt-dlp status:", e);
  } finally {
    ytdlpChecking.value = false;
  }
}

async function handleDownloadYtdlp() {
  ytdlpDownloading.value = true;
  ytdlpDownloadPercent.value = 0;
  const unlisten = await listen<DownloadProgress>(
    "ytdlp-download-progress",
    (event) => {
      ytdlpDownloadPercent.value = event.payload.percent;
    },
  );
  try {
    await invoke("download_ytdlp");
    window.$message.success("yt-dlp 下载完成");
    await checkYtdlpStatus();
  } catch (e: any) {
    window.$message.error(`下载失败: ${e}`);
  } finally {
    unlisten();
    ytdlpDownloading.value = false;
  }
}

async function handleUpdateYtdlp() {
  ytdlpUpdating.value = true;
  try {
    const result = await invoke<string>("update_ytdlp");
    if (result.includes("up to date")) {
      window.$message.success("已是最新版本");
    } else if (result.includes("Updated")) {
      window.$message.success("已更新到最新版本");
      await checkYtdlpStatus();
    } else {
      window.$message.success("已是最新版本");
    }
  } catch (e: any) {
    window.$message.error(`更新失败: ${e}`);
  } finally {
    ytdlpUpdating.value = false;
  }
}

// ========== Deno ==========
const denoStatus = ref<DenoStatus | null>(null);
const denoChecking = ref(true);
const denoDownloading = ref(false);
const denoDownloadPercent = ref(0);

async function checkDenoStatus() {
  denoChecking.value = true;
  try {
    denoStatus.value = await invoke<DenoStatus>("get_deno_status");
  } catch (e) {
    console.error("Failed to check Deno status:", e);
  } finally {
    denoChecking.value = false;
  }
}

async function handleDownloadDeno() {
  denoDownloading.value = true;
  denoDownloadPercent.value = 0;
  const unlisten = await listen<DownloadProgress>(
    "deno-download-progress",
    (event) => {
      denoDownloadPercent.value = event.payload.percent;
    },
  );
  try {
    await invoke("download_deno");
    window.$message.success("Deno 下载完成");
    await checkDenoStatus();
  } catch (e: any) {
    window.$message.error(`下载失败: ${e}`);
  } finally {
    unlisten();
    denoDownloading.value = false;
  }
}

// ========== 刷新 ==========
function refreshAll() {
  checkYtdlpStatus();
  checkDenoStatus();
}

onMounted(async () => {
  platform.value = await invoke<string>("get_platform");
  refreshAll();
});
</script>

<template>
  <div class="settings-page">
    <div class="page-header">
      <n-h2 style="margin: 0">设置</n-h2>
      <n-button size="small" quaternary @click="refreshAll">刷新</n-button>
    </div>

    <!-- yt-dlp -->
    <n-card title="yt-dlp" size="small" class="section-card">
      <template #header-extra>
        <n-flex align="center" :size="8">
          <n-tag
            v-if="!ytdlpChecking"
            :type="ytdlpStatus?.installed ? 'success' : 'error'"
            size="small"
            round
          >
            {{ ytdlpStatus?.installed ? "已安装" : "未安装" }}
          </n-tag>
          <n-button
            v-if="ytdlpStatus?.installed"
            size="tiny"
            :loading="ytdlpUpdating"
            @click="handleUpdateYtdlp"
          >
            检查更新
          </n-button>
          <n-button
            v-if="ytdlpStatus && !ytdlpStatus.installed"
            type="primary"
            size="tiny"
            :loading="ytdlpDownloading"
            :disabled="ytdlpDownloading"
            @click="handleDownloadYtdlp"
          >
            下载
          </n-button>
        </n-flex>
      </template>

      <n-spin :show="ytdlpChecking">
        <n-flex vertical :size="12">
          <n-text depth="3" style="font-size: 13px">
            本软件所需核心工具
          </n-text>

          <div class="info-list">
            <div class="info-row">
              <span class="info-label">版本</span>
              <n-text code>{{ ytdlpStatus?.version || "—" }}</n-text>
            </div>
            <div class="info-row">
              <span class="info-label">路径</span>
              <n-ellipsis :line-clamp="1" :tooltip="{ width: 360 }">
                {{ ytdlpStatus?.path || "—" }}
              </n-ellipsis>
            </div>
          </div>

          <n-collapse-transition :show="ytdlpDownloading">
            <n-progress
              type="line"
              :percentage="Math.round(ytdlpDownloadPercent)"
              :processing="true"
              indicator-placement="inside"
              :height="20"
              :border-radius="4"
              style="margin-top: 4px"
            />
          </n-collapse-transition>
        </n-flex>
      </n-spin>
    </n-card>

    <!-- Deno -->
    <n-card title="Deno JS 运行时" size="small" class="section-card">
      <template #header-extra>
        <n-flex align="center" :size="8">
          <n-tag
            v-if="!denoChecking"
            :type="denoStatus?.installed ? 'success' : 'warning'"
            size="small"
            round
          >
            {{ denoStatus?.installed ? "已安装" : "未安装" }}
          </n-tag>
          <n-button
            v-if="denoStatus && !denoStatus.installed"
            type="primary"
            size="tiny"
            :loading="denoDownloading"
            :disabled="denoDownloading"
            @click="handleDownloadDeno"
          >
            下载
          </n-button>
        </n-flex>
      </template>

      <n-spin :show="denoChecking">
        <n-flex vertical :size="12">
          <n-text depth="3" style="font-size: 13px">
            YouTube 需要 JavaScript
            运行时来解析视频信息。未安装时部分格式可能缺失或者无法下载
          </n-text>

          <div class="info-list">
            <div class="info-row">
              <span class="info-label">版本</span>
              <n-text code>{{ denoStatus?.version || "—" }}</n-text>
            </div>
            <div class="info-row">
              <span class="info-label">路径</span>
              <n-ellipsis :line-clamp="1" :tooltip="{ width: 360 }">
                {{ denoStatus?.path || "—" }}
              </n-ellipsis>
            </div>
          </div>

          <n-collapse-transition :show="denoDownloading">
            <n-progress
              type="line"
              :percentage="Math.round(denoDownloadPercent)"
              :processing="true"
              indicator-placement="inside"
              :height="20"
              :border-radius="4"
              style="margin-top: 4px"
            />
          </n-collapse-transition>
        </n-flex>
      </n-spin>
    </n-card>

    <!-- 外观 -->
    <n-card title="外观" size="small" class="section-card">
      <div class="info-list">
        <div class="info-row">
          <span class="info-label">主题模式</span>
          <n-select
            v-model:value="settingStore.themeMode"
            :options="themeModeOptions"
            style="width: 120px"
            size="small"
          />
        </div>
      </div>
    </n-card>

    <!-- 关于 -->
    <n-card title="关于" size="small" class="section-card">
      <n-flex vertical :size="8">
        <n-text depth="3" style="font-size: 13px">
          基于 Tauri 2 + Vue 3 + Naive UI 构建的 yt-dlp 图形界面工具。
        </n-text>
        <div class="info-list">
          <div class="info-row">
            <span class="info-label">版本</span>
            <n-text code>v0.1.0</n-text>
          </div>
          <div class="info-row">
            <span class="info-label">平台</span>
            <n-text code>{{ platformLabel }}</n-text>
          </div>
        </div>
      </n-flex>
    </n-card>
  </div>
</template>

<style scoped lang="scss">
.settings-page {
  max-width: 100%;
}

.page-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 20px;
}

.section-card {
  margin-bottom: 12px;
}

.info-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.info-row {
  display: flex;
  align-items: center;
  font-size: 13px;
  min-height: 28px;

  // 虚线填充 label 和 value 之间的空隙
  &::before {
    order: 1;
    content: "";
    flex: 1;
    border-bottom: 1px dashed var(--n-border-color, #e0e0e6);
    margin: 0 8px;
    min-width: 20px;
  }

  // value 排到最右
  > :last-child {
    order: 2;
    flex-shrink: 0;
  }
}

.info-label {
  color: var(--n-text-color-3, #999);
  flex-shrink: 0;
  order: 0;
}

</style>
