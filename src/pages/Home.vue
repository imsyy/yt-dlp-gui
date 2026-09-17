<script setup lang="ts">
import { readText } from "@tauri-apps/plugin-clipboard-manager";
import { isValidUrl } from "@/utils/validate";
import { useVideoStore } from "@/stores/video";
import { createPendingItem, usePendingStore } from "@/stores/pending";
import { useHistoryStore } from "@/stores/history";
import { useSettingStore } from "@/stores/setting";
import { useDownloadLauncher } from "@/composables/useDownloadLauncher";
import { useI18n } from "vue-i18n";
import type { FetchedVideoData, HomeMode } from "@/types";

const { t: translate, tm: translateMessage } = useI18n();
const router = useRouter();
const route = useRoute();
const videoStore = useVideoStore();
const pendingStore = usePendingStore();
const historyStore = useHistoryStore();
const settingStore = useSettingStore();
const { createPreparingTask, markPreparationError, launchDownload } = useDownloadLauncher();

const url = ref("");
const batchInput = ref("");
const batchParsing = ref(false);
const showQuickSettings = ref(false);
const BATCH_LIMIT = 50;

/**
 * 从多行或空格分隔的文本中提取并去重有效网址
 *
 * @param sourceText 输入的文本内容
 * @returns 提取到的有效 URL 字符串列表
 */
const extractUrls = (sourceText: string): string[] =>
  Array.from(
    new Set(
      sourceText
        .split(/\s+/)
        .map((item) => item.trim())
        .filter(isValidUrl),
    ),
  );

const batchUrls = computed(() => extractUrls(batchInput.value));
const isBusy = computed(() => videoStore.fetching || batchParsing.value);

const historyIndex = ref(-1);
const showHistory = ref(false);

/**
 * 监听标准模式输入框键盘事件（Enter 触发搜索，上下键翻查历史记录）
 *
 * @param keyboardEvent 键盘按键事件对象
 */
const handleKeydown = (keyboardEvent: KeyboardEvent): void => {
  if (keyboardEvent.key === "Enter") {
    handleSearch();
    return;
  }

  if (historyStore.urls.length === 0) return;

  if (keyboardEvent.key === "ArrowUp") {
    keyboardEvent.preventDefault();
    if (historyIndex.value < historyStore.urls.length - 1) {
      historyIndex.value++;
    }
    url.value = historyStore.urls[historyIndex.value];
  } else if (keyboardEvent.key === "ArrowDown") {
    keyboardEvent.preventDefault();
    if (historyIndex.value > 0) {
      historyIndex.value--;
      url.value = historyStore.urls[historyIndex.value];
    } else {
      historyIndex.value = -1;
      url.value = "";
    }
  }
};

/**
 * 输入框内容变动时重置历史记录索引游标
 */
const handleInput = (): void => {
  historyIndex.value = -1;
};

/**
 * 从历史记录抽屉中选中指定 URL 填充到输入框并关闭抽屉
 *
 * @param selectedUrl 用户选中的历史 URL
 */
const selectHistory = (selectedUrl: string): void => {
  url.value = selectedUrl;
  showHistory.value = false;
  historyIndex.value = -1;
};

/**
 * 读取系统剪贴板内容并根据当前模式自动填充到对应输入框
 *
 * @returns Promise<void>
 */
const handlePaste = async (): Promise<void> => {
  try {
    const clipboardContent = await readText();
    const trimmedContent = clipboardContent.trim();
    if (!trimmedContent) {
      window.$message.warning(translate("clipboard.empty"));
      return;
    }
    if (settingStore.homeMode === "batch") {
      const parsedUrls = extractUrls(trimmedContent);
      if (parsedUrls.length === 0) {
        window.$message.warning(translate("clipboard.invalidUrl"));
        return;
      }
      batchInput.value = parsedUrls.join("\n");
      window.$message.success(translate("home.batchPasted", { count: parsedUrls.length }));
    } else {
      if (!isValidUrl(trimmedContent)) {
        window.$message.warning(translate("clipboard.invalidUrl"));
        return;
      }
      url.value = trimmedContent;
      historyIndex.value = -1;
      window.$message.success(translate("clipboard.pasteSuccess"));
    }
  } catch {
    window.$message.warning(translate("clipboard.readFailed"));
  }
};

/**
 * 将毫秒时间戳转换为易读的历史时间字符串（如“今天 14:30”或日期时间）
 *
 * @param historyTimestamp 历史记录产生的时间戳
 * @returns 格式化后的时间字符串
 */
const formatHistoryTime = (historyTimestamp: number): string => {
  if (!historyTimestamp) return "";
  const now = new Date();
  const targetDate = new Date(historyTimestamp);
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const targetDay = new Date(targetDate.getFullYear(), targetDate.getMonth(), targetDate.getDate());
  const dayDifference = (today.getTime() - targetDay.getTime()) / 86400000;
  const timeString = `${String(targetDate.getHours()).padStart(2, "0")}:${String(targetDate.getMinutes()).padStart(2, "0")}`;

  if (dayDifference === 0) return `${translate("downloads.today")} ${timeString}`;
  if (dayDifference === 1) return `${translate("downloads.yesterday")} ${timeString}`;
  if (dayDifference === 2) return `${translate("downloads.dayBeforeYesterday")} ${timeString}`;
  return `${targetDate.getFullYear()}/${String(targetDate.getMonth() + 1).padStart(2, "0")}/${String(targetDate.getDate()).padStart(2, "0")} ${timeString}`;
};

const currentTipIndex = ref(0);
let tipTimer: ReturnType<typeof setInterval> | null = null;

/**
 * 响应来自深链接、命令行或浏览器扩展的外部 URL 导入：
 * 自动识别并适配标准模式与批量模式：
 * 1. 明确指定 batch 模式或导入多个链接时，自动切入批量模式并去重追加到批量输入框；
 * 2. 明确指定 standard 模式时，切入标准模式并自动执行单条解析；
 * 3. 未显式指定模式时，尊重当前客户端所处的模式，避免在批量模式下打乱用户界面。
 *
 * @param routeQuery 包含 url、urls、mode 等参数的路由 Query 对象
 */
const applyExternalImport = (routeQuery: Record<string, unknown>): void => {
  const targetMode = routeQuery.mode as HomeMode | undefined;
  let importedUrls: string[] = [];

  if (typeof routeQuery.urls === "string") {
    try {
      const parsedUrlArray = JSON.parse(routeQuery.urls);
      if (Array.isArray(parsedUrlArray)) {
        importedUrls = parsedUrlArray.filter(
          (urlCandidate): urlCandidate is string =>
            typeof urlCandidate === "string" && isValidUrl(urlCandidate),
        );
      }
    } catch {
      importedUrls = extractUrls(routeQuery.urls);
    }
  }

  if (typeof routeQuery.url === "string") {
    const singleUrl = routeQuery.url.trim();
    if (isValidUrl(singleUrl) && !importedUrls.includes(singleUrl)) {
      importedUrls.push(singleUrl);
    }
  }

  if (importedUrls.length === 0) return;

  router.replace({ name: "home", query: {} });

  const effectiveMode: HomeMode =
    targetMode || (importedUrls.length > 1 ? "batch" : settingStore.homeMode);

  if (effectiveMode === "batch") {
    settingStore.homeMode = "batch";
    const existingUrls = extractUrls(batchInput.value);
    const mergedUrls = Array.from(new Set([...existingUrls, ...importedUrls]));
    batchInput.value = mergedUrls.join("\n");
    window.$message.success(translate("home.batchPasted", { count: importedUrls.length }));
  } else {
    settingStore.homeMode = "standard";
    url.value = importedUrls[0];
    historyIndex.value = -1;
    handleSearch();
  }
};

onMounted(() => {
  tipTimer = setInterval(() => {
    const tipsList = translateMessage("home.tips") as string[];
    currentTipIndex.value = (currentTipIndex.value + 1) % tipsList.length;
  }, 4000);
  if (route.query.url || route.query.urls) {
    applyExternalImport(route.query);
  }
});

// 监听 query 变化（已在首页时收到新导入或深链接）
watch(
  () => [route.query.url, route.query.urls, route.query._t],
  ([newUrl, newUrls]) => {
    if (newUrl || newUrls) {
      applyExternalImport(route.query);
    }
  },
);

onUnmounted(() => {
  if (tipTimer) clearInterval(tipTimer);
});

/**
 * 处理视频解析成功后的数据分发（加入待下载列表或发起快速下载任务）
 *
 * @param videoData 解析成功的视频详细元数据
 * @param preparingTaskId 提前占位的准备中任务 ID（可选）
 * @returns Promise<boolean> 是否成功提交处理
 */
const handleParsedData = async (
  videoData: FetchedVideoData,
  preparingTaskId?: string,
): Promise<boolean> => {
  if (settingStore.homeDownloadBehavior === "pending") {
    pendingStore.add(videoData);
    return true;
  }

  const downloadResult = await launchDownload(createPendingItem(videoData, true), preparingTaskId);
  return downloadResult === "started" || downloadResult === "queued";
};

/**
 * 校验快速下载模式所需配置（如下载路径是否已设置）
 *
 * @returns boolean 配置是否就绪
 */
const ensureQuickDownloadConfigured = (): boolean => {
  if (settingStore.homeDownloadBehavior !== "quick" || settingStore.downloadDir) return true;
  window.$message.warning(translate("detail.setDownloadDirFirst"));
  showQuickSettings.value = true;
  return false;
};

/**
 * 执行标准模式下的单条视频链接解析与下载提交流程
 *
 * @returns Promise<void>
 */
const handleSearch = async (): Promise<void> => {
  const trimmedUrl = url.value.trim();
  if (!trimmedUrl) return;
  if (!isValidUrl(trimmedUrl)) {
    window.$message.warning(translate("home.enterValidUrl"));
    return;
  }
  if (!ensureQuickDownloadConfigured()) return;
  const preparingTaskId =
    settingStore.homeDownloadBehavior === "quick" ? createPreparingTask(trimmedUrl) : undefined;
  if (preparingTaskId) await router.push({ name: "downloads" });
  const fetchedData = await videoStore.fetchVideoInfo(trimmedUrl);
  if (fetchedData) {
    historyStore.add(trimmedUrl, fetchedData.videoInfo.title);
    const hasSubmitted = await handleParsedData(fetchedData, preparingTaskId);
    if (hasSubmitted && settingStore.homeDownloadBehavior === "pending") {
      router.push({ name: "pending" });
    }
  } else if (preparingTaskId) {
    markPreparationError(preparingTaskId);
  }
};

/**
 * 批量解析去重后的链接列表（逐项顺序解析以防瞬时占用过多系统进程）
 *
 * @returns Promise<void>
 */
const handleBatchSearch = async (): Promise<void> => {
  const targetUrls = batchUrls.value;
  if (targetUrls.length === 0) {
    window.$message.warning(translate("home.batchEmpty"));
    return;
  }
  if (targetUrls.length > BATCH_LIMIT) {
    window.$message.warning(translate("home.batchLimit", { count: BATCH_LIMIT }));
    return;
  }
  if (!ensureQuickDownloadConfigured()) return;

  batchParsing.value = true;
  let succeededCount = 0;
  const preparingTasks =
    settingStore.homeDownloadBehavior === "quick"
      ? new Map(targetUrls.map((targetUrl) => [targetUrl, createPreparingTask(targetUrl)]))
      : new Map<string, string>();
  if (preparingTasks.size > 0) await router.push({ name: "downloads" });

  try {
    for (const targetUrl of targetUrls) {
      const fetchedData = await videoStore.fetchVideoInfo(targetUrl, { silent: true });
      if (fetchedData) {
        historyStore.add(targetUrl, fetchedData.videoInfo.title);
        if (await handleParsedData(fetchedData, preparingTasks.get(targetUrl))) {
          succeededCount += 1;
        }
      } else {
        const preparingTaskId = preparingTasks.get(targetUrl);
        if (preparingTaskId) markPreparationError(preparingTaskId);
      }
    }
  } finally {
    batchParsing.value = false;
  }

  if (succeededCount > 0) {
    window.$message.success(
      translate("home.batchComplete", {
        succeeded: succeededCount,
        failed: targetUrls.length - succeededCount,
      }),
    );
    batchInput.value = "";
    if (settingStore.homeDownloadBehavior === "pending") router.push({ name: "pending" });
  } else {
    window.$message.error(translate("home.batchAllFailed"));
  }
};
</script>

<template>
  <div class="home-page">
    <n-flex vertical align="center" justify="center" :size="20" class="search-view">
      <n-flex vertical align="center" :size="8">
        <n-flex align="center" class="hero-logo">
          <span class="hero-text">YDL GUI</span>
        </n-flex>
        <n-text depth="3" style="font-size: 16px">
          {{ $t("home.slogan") }}
        </n-text>
      </n-flex>
      <n-flex :size="8">
        <n-button
          size="small"
          strong
          secondary
          round
          :disabled="isBusy"
          :type="settingStore.homeMode === 'standard' ? 'primary' : 'default'"
          @click="settingStore.homeMode = 'standard'"
        >
          {{ $t("home.standardMode") }}
        </n-button>
        <n-button
          size="small"
          strong
          secondary
          round
          :disabled="isBusy"
          :type="settingStore.homeMode === 'batch' ? 'primary' : 'default'"
          @click="settingStore.homeMode = 'batch'"
        >
          {{ $t("home.batchMode") }}
        </n-button>
      </n-flex>

      <div class="input-stage" :class="{ 'is-batch': settingStore.homeMode === 'batch' }">
        <Transition name="mode-fade">
          <div v-if="settingStore.homeMode === 'standard'" key="standard" class="input-panel">
            <n-input
              v-model:value="url"
              :placeholder="$t('home.inputPlaceholder')"
              size="large"
              round
              clearable
              :disabled="isBusy"
              @keydown="handleKeydown"
              @input="handleInput"
            />

            <div class="submit-row">
              <div class="submit-left">
                <DownloadBehaviorControls
                  v-model="settingStore.homeDownloadBehavior"
                  @settings="showQuickSettings = true"
                />
              </div>

              <n-button
                type="primary"
                strong
                secondary
                round
                :loading="videoStore.fetching"
                :disabled="!url.trim() || isBusy"
                @click="handleSearch"
              >
                <template #icon>
                  <n-icon>
                    <icon-mdi-download v-if="settingStore.homeDownloadBehavior === 'quick'" />
                    <icon-mdi-magnify v-else />
                  </n-icon>
                </template>
                {{
                  settingStore.homeDownloadBehavior === "quick"
                    ? $t("common.download")
                    : $t("home.parse")
                }}
              </n-button>
            </div>
          </div>

          <div v-else key="batch" class="input-panel">
            <n-input
              v-model:value="batchInput"
              type="textarea"
              :placeholder="$t('home.batchPlaceholder')"
              :autosize="{ minRows: 3, maxRows: 3 }"
              :disabled="isBusy"
              @keydown.ctrl.enter.prevent="handleBatchSearch"
            />

            <div class="submit-row">
              <div class="submit-left">
                <DownloadBehaviorControls
                  v-model="settingStore.homeDownloadBehavior"
                  @settings="showQuickSettings = true"
                />
              </div>

              <n-button
                type="primary"
                strong
                secondary
                round
                :loading="batchParsing"
                :disabled="batchUrls.length === 0 || isBusy"
                @click="handleBatchSearch"
              >
                <template #icon>
                  <n-icon>
                    <icon-mdi-download v-if="settingStore.homeDownloadBehavior === 'quick'" />
                    <icon-mdi-magnify v-else />
                  </n-icon>
                </template>
                {{
                  settingStore.homeDownloadBehavior === "quick"
                    ? $t("common.download")
                    : $t("home.parseBatch", { count: batchUrls.length })
                }}
              </n-button>
            </div>
          </div>
        </Transition>
      </div>
      <n-flex :size="8" justify="center">
        <n-button size="small" strong secondary round @click="handlePaste">
          <template #icon>
            <n-icon size="14"><icon-mdi-content-paste /></n-icon>
          </template>
          {{ $t("common.paste") }}
        </n-button>
        <n-button
          size="small"
          strong
          secondary
          round
          :disabled="historyStore.items.length === 0"
          @click="showHistory = true"
        >
          <template #icon>
            <n-icon size="14"><icon-mdi-history /></n-icon>
          </template>
          {{ $t("home.parseHistory") }}
        </n-button>
      </n-flex>
      <div class="tips-container">
        <Transition name="tip-fade" mode="out-in">
          <n-text :key="currentTipIndex" depth="3" class="tip-item">
            {{ $t(`home.tips[${currentTipIndex}]`) }}
          </n-text>
        </Transition>
      </div>
    </n-flex>

    <n-drawer v-model:show="showHistory" :width="360" placement="right">
      <n-drawer-content :native-scrollbar="false">
        <template #header>
          <n-flex align="center" justify="space-between" style="width: 100%">
            <span>{{ $t("home.parseHistory") }}</span>
            <n-button
              size="tiny"
              strong
              secondary
              type="error"
              :disabled="historyStore.items.length === 0"
              @click="historyStore.clear()"
            >
              {{ $t("common.clear") }}
            </n-button>
          </n-flex>
        </template>
        <n-empty
          v-if="historyStore.items.length === 0"
          :description="$t('home.noHistory')"
          style="margin-top: 80px"
        />
        <n-list v-else bordered clickable>
          <n-list-item
            v-for="(item, index) in historyStore.items"
            :key="index"
            @click="selectHistory(item.url)"
          >
            <n-flex vertical :size="2" style="min-width: 0">
              <n-flex :size="4" :wrap="false" align="center" style="min-width: 0">
                <n-ellipsis :line-clamp="1" :tooltip="false" class="history-title">
                  {{ item.title || item.url }}
                </n-ellipsis>
              </n-flex>
              <n-flex :size="8" :wrap="false" align="center">
                <n-text depth="3" class="history-url">
                  <n-ellipsis :line-clamp="1" :tooltip="false">
                    {{ item.url }}
                  </n-ellipsis>
                </n-text>
                <n-text depth="3" class="history-time">
                  {{ formatHistoryTime(item.time) }}
                </n-text>
              </n-flex>
            </n-flex>
            <template #suffix>
              <n-button
                quaternary
                circle
                size="tiny"
                class="history-delete"
                @click.stop="historyStore.remove(item.url)"
              >
                <template #icon>
                  <n-icon size="14"><icon-mdi-close /></n-icon>
                </template>
              </n-button>
            </template>
          </n-list-item>
        </n-list>
      </n-drawer-content>
    </n-drawer>

    <QuickDownloadSettingsModal v-model:show="showQuickSettings" />
  </div>
</template>

<style scoped lang="scss">
.home-page {
  height: 100%;
  position: relative;
  overflow-y: auto;
  padding: 16px;
}

.search-view {
  height: 100%;
  min-height: 300px;
  padding-bottom: 40px;

  .hero-logo {
    user-select: none;

    .hero-text {
      font-weight: 800;
      font-size: 28px;
      letter-spacing: 1px;
    }
  }

  .search-bar {
    width: 100%;
    max-width: 500px;
  }
}

.input-stage {
  position: relative;
  width: 100%;
  max-width: 500px;
  height: 88px;
  transition-property: height;
  transition-duration: 180ms;
  transition-timing-function: cubic-bezier(0.2, 0, 0, 1);
}

.input-stage.is-batch {
  height: 132px;
}

.input-panel {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.submit-row {
  min-height: 34px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.submit-left {
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 8px;
}

.mode-fade-enter-active,
.mode-fade-leave-active {
  transition-property: opacity, transform;
}

.mode-fade-enter-active {
  transition-duration: 160ms;
  transition-timing-function: ease-out;
}

.mode-fade-leave-active {
  transition-duration: 120ms;
  transition-timing-function: ease-in;
}

.mode-fade-enter-from {
  opacity: 0;
  transform: translateY(4px);
}

.mode-fade-leave-to {
  opacity: 0;
  transform: translateY(-4px);
}

.tips-container {
  width: 100%;
  max-width: 500px;
  text-align: center;
  height: 20px;
  position: relative;
  margin-top: -8px;

  .tip-item {
    font-size: 12px;
    display: inline-block;
  }
}

.history-title {
  font-size: 13px;
  font-weight: 500;
  flex: 1;
  min-width: 0;
}

.history-url {
  font-size: 11px;
  flex: 1;
  min-width: 0;
}

.history-time {
  font-size: 11px;
  white-space: nowrap;
  flex-shrink: 0;
}

.history-delete {
  opacity: 0;
  flex-shrink: 0;
  transition: opacity 0.15s;
}

:deep(.n-list-item):hover .history-delete {
  opacity: 1;
}
</style>
