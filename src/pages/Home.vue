<script setup lang="ts">
import { readText } from "@tauri-apps/plugin-clipboard-manager";
import { isValidUrl } from "@/utils/validate";
import { useVideoStore } from "@/stores/video";
import { useHistoryStore } from "@/stores/history";
import { useI18n } from "vue-i18n";

const { t, tm } = useI18n();
const router = useRouter();
const videoStore = useVideoStore();
const historyStore = useHistoryStore();

const url = ref("");

const historyIndex = ref(-1);
const showHistory = ref(false);

const handleKeydown = (e: KeyboardEvent) => {
  if (e.key === "Enter") {
    handleSearch();
    return;
  }

  if (historyStore.urls.length === 0) return;

  if (e.key === "ArrowUp") {
    e.preventDefault();
    if (historyIndex.value < historyStore.urls.length - 1) {
      historyIndex.value++;
    }
    url.value = historyStore.urls[historyIndex.value];
  } else if (e.key === "ArrowDown") {
    e.preventDefault();
    if (historyIndex.value > 0) {
      historyIndex.value--;
      url.value = historyStore.urls[historyIndex.value];
    } else {
      historyIndex.value = -1;
      url.value = "";
    }
  }
};

const handleInput = () => {
  historyIndex.value = -1;
};

const selectHistory = (item: string) => {
  url.value = item;
  showHistory.value = false;
  historyIndex.value = -1;
};

const handlePaste = async () => {
  try {
    const text = await readText();
    const trimmed = text.trim();
    if (!trimmed) {
      window.$message.warning(t("clipboard.empty"));
      return;
    }
    if (!isValidUrl(trimmed)) {
      window.$message.warning(t("clipboard.invalidUrl"));
      return;
    }
    url.value = trimmed;
    historyIndex.value = -1;
    window.$message.success(t("clipboard.pasteSuccess"));
  } catch {
    window.$message.warning(t("clipboard.readFailed"));
  }
};

/** 格式化历史记录时间 */
const formatHistoryTime = (time: number): string => {
  if (!time) return "";
  const now = new Date();
  const d = new Date(time);
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const target = new Date(d.getFullYear(), d.getMonth(), d.getDate());
  const diff = (today.getTime() - target.getTime()) / 86400000;
  const timeStr = `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;

  if (diff === 0) return `${t("downloads.today")} ${timeStr}`;
  if (diff === 1) return `${t("downloads.yesterday")} ${timeStr}`;
  if (diff === 2) return `${t("downloads.dayBeforeYesterday")} ${timeStr}`;
  return `${d.getFullYear()}/${String(d.getMonth() + 1).padStart(2, "0")}/${String(d.getDate()).padStart(2, "0")} ${timeStr}`;
};

const currentTipIndex = ref(0);
let tipTimer: ReturnType<typeof setInterval> | null = null;

const route = useRoute();

onMounted(() => {
  tipTimer = setInterval(() => {
    const tips = tm("home.tips");
    currentTipIndex.value = (currentTipIndex.value + 1) % tips.length;
  }, 4000);
  // 从深链接 query 参数自动填充 URL 并触发解析
  const deepLinkUrl = route.query.url as string | undefined;
  if (deepLinkUrl) {
    url.value = deepLinkUrl;
    router.replace({ name: "home", query: {} });
    handleSearch();
  }
});

// 监听 query 变化（已在首页时收到新深链接）
watch(() => route.query.url, (newUrl) => {
  if (newUrl && typeof newUrl === "string") {
    url.value = newUrl;
    router.replace({ name: "home", query: {} });
    handleSearch();
  }
});

onUnmounted(() => {
  if (tipTimer) clearInterval(tipTimer);
});

/** 解析视频链接，获取视频信息与可用格式 */
const handleSearch = async () => {
  const trimmed = url.value.trim();
  if (!trimmed) return;
  if (!isValidUrl(trimmed)) {
    window.$message.warning(t("home.enterValidUrl"));
    return;
  }
  const success = await videoStore.fetchVideoInfo(trimmed);
  if (success) {
    historyStore.add(trimmed, videoStore.videoInfo?.title);
    router.push({ name: "detail" });
  }
};
</script>

<template>
  <div class="home-page">
    <n-flex vertical align="center" justify="center" :size="20" class="search-view">
      <n-flex vertical align="center" :size="8">
        <n-flex align="center" :size="8" class="hero-logo">
          <icon-mdi-youtube class="hero-icon" />
          <span class="hero-text">GUI</span>
        </n-flex>
        <n-text depth="3" style="font-size: 16px">
          {{ $t('home.slogan') }}
        </n-text>
      </n-flex>
      <n-flex :size="8" :wrap="false" class="search-bar">
        <n-input
          v-model:value="url"
          :placeholder="$t('home.inputPlaceholder')"
          size="large"
          round
          clearable
          :disabled="videoStore.fetching"
          @keydown="handleKeydown"
          @input="handleInput"
        />
        <n-button
          type="primary"
          size="large"
          round
          strong
          secondary
          :loading="videoStore.fetching"
          :disabled="!url.trim()"
          @click="handleSearch"
        >
          <template #icon>
            <n-icon>
              <icon-mdi-magnify />
            </n-icon>
          </template>
          {{ $t('home.parse') }}
        </n-button>
      </n-flex>
      <n-flex :size="8" justify="center">
        <n-button size="small" strong secondary round @click="handlePaste">
          <template #icon>
            <n-icon size="14"><icon-mdi-content-paste /></n-icon>
          </template>
          {{ $t('home.pasteFromClipboard') }}
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
          {{ $t('home.parseHistory') }}
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
            <span>{{ $t('home.parseHistory') }}</span>
            <n-button
              size="tiny"
              strong
              secondary
              type="error"
              :disabled="historyStore.items.length === 0"
              @click="historyStore.clear()"
            >
              {{ $t('common.clear') }}
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
  </div>
</template>

<style scoped lang="scss">
.home-page {
  height: 100%;
  position: relative;
}

.search-view {
  height: 100%;
  min-height: 300px;

  .hero-logo {
    user-select: none;

    .hero-icon {
      font-size: 48px;
    }

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
