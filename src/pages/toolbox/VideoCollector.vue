<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useI18n } from "vue-i18n";
import { goToolList } from "@/utils/toolbox";
import IconMdiOpenInApp from "~icons/mdi/open-in-app";
import IconMdiDownload from "~icons/mdi/download";

const { t } = useI18n();
const router = useRouter();

/** 采集窗口回传的进度事件载荷 */
interface CollectorProgress {
  scanning: boolean;
  count: number;
  scans: number;
}

/** 采集窗口回传的结果事件载荷 */
interface CollectorResult {
  urls: string[];
  /** 结果同时保存到的本地 txt 路径（下载通道产物） */
  file?: string;
}

/** 采集窗口回传的日志事件载荷 */
interface CollectorLog {
  message: string;
}

const opening = ref(false);
const scanning = ref(false);
const collectedCount = ref(0);
const scanRounds = ref(0);
const collectedUrls = ref<string[]>([]);
const savedFile = ref("");
const logs = ref<string[]>([]);

const unlisteners: UnlistenFn[] = [];

/** 追加一条采集窗口日志（仅保留最近 30 条，避免无限增长） */
const pushLog = (message: string) => {
  logs.value.unshift(message);
  if (logs.value.length > 30) logs.value.length = 30;
};

/** 构造随应用语言本地化的悬浮面板文案，注入到采集窗口 */
const buildPanelText = (): string =>
  JSON.stringify({
    title: t("collector.panelTitle"),
    hint: t("collector.panelHint"),
    start: t("collector.start"),
    stop: t("collector.stop"),
    send: t("collector.send"),
    started: t("collector.logStarted"),
    stoppedByUser: t("collector.logStopped"),
    noMore: t("collector.logNoMore"),
    done: t("collector.logDone", { count: "{count}" }),
    sent: t("collector.logSent", { count: "{count}" }),
    emptyResult: t("collector.logEmpty"),
  });

/** 打开（或聚焦）采集窗口 */
const openCollector = async () => {
  opening.value = true;
  try {
    await invoke("open_collector_window", { panelText: buildPanelText() });
    window.$message.success(t("collector.opened"));
  } catch (error: unknown) {
    window.$message.error(t("collector.openFailed", { e: String(error) }));
  } finally {
    opening.value = false;
  }
};

/** 把采集到的链接送入首页批量模式 */
const sendToBatch = () => {
  if (collectedUrls.value.length === 0) {
    window.$message.warning(t("collector.nothingToSend"));
    return;
  }
  void router.push({
    name: "home",
    query: {
      urls: JSON.stringify(collectedUrls.value),
      mode: "batch",
      _t: String(Date.now()),
    },
  });
};

onMounted(async () => {
  unlisteners.push(
    await listen<CollectorProgress>("fb-collector-progress", (event) => {
      scanning.value = event.payload.scanning;
      collectedCount.value = event.payload.count;
      if (event.payload.scans > 0) scanRounds.value = event.payload.scans;
    }),
  );
  unlisteners.push(
    await listen<CollectorResult>("fb-collector-result", (event) => {
      collectedUrls.value = event.payload.urls;
      savedFile.value = event.payload.file ?? "";
      collectedCount.value = event.payload.urls.length;
      window.$message.success(t("collector.received", { count: event.payload.urls.length }));
    }),
  );
  unlisteners.push(
    await listen<CollectorLog>("fb-collector-log", (event) => {
      pushLog(event.payload.message);
    }),
  );
});

onUnmounted(() => {
  for (const unlisten of unlisteners) unlisten();
});
</script>

<template>
  <n-flex vertical :size="12">
    <n-flex align="center" justify="space-between" :wrap="false">
      <n-flex align="center" :size="8">
        <n-button strong secondary size="small" @click="goToolList(router)">
          <template #icon>
            <n-icon><icon-mdi-arrow-left /></n-icon>
          </template>
          {{ $t("common.back") }}
        </n-button>
        <n-text strong style="font-size: 15px">{{ $t("collector.title") }}</n-text>
        <n-tag size="small" round :bordered="false" type="warning">
          {{ $t("browserExt.tagBeta") }}
        </n-tag>
      </n-flex>
      <n-button type="primary" size="small" :loading="opening" @click="openCollector">
        <template #icon>
          <n-icon><icon-mdi-open-in-app /></n-icon>
        </template>
        {{ $t("collector.openWindow") }}
      </n-button>
    </n-flex>

    <n-alert type="info" :bordered="false">{{ $t("collector.intro") }}</n-alert>

    <n-card size="small" :title="$t('collector.stepsHeading')">
      <ol class="steps">
        <li v-for="index in 4" :key="index">{{ $t(`collector.step${index}`) }}</li>
      </ol>
    </n-card>

    <n-card size="small">
      <n-flex align="center" justify="space-between" :wrap="false">
        <n-flex align="center" :size="16">
          <n-statistic :label="$t('collector.collected')" :value="collectedCount" />
          <n-statistic :label="$t('collector.scanRounds')" :value="scanRounds" />
          <n-tag :type="scanning ? 'success' : 'default'" size="small" round :bordered="false">
            {{ scanning ? $t("collector.statusScanning") : $t("collector.statusIdle") }}
          </n-tag>
        </n-flex>
        <n-button
          type="primary"
          strong
          secondary
          :disabled="collectedUrls.length === 0"
          @click="sendToBatch"
        >
          <template #icon>
            <n-icon><icon-mdi-download /></n-icon>
          </template>
          {{ $t("collector.sendToBatch", { count: collectedUrls.length }) }}
        </n-button>
      </n-flex>
    </n-card>

    <n-card v-if="savedFile" size="small" title="TXT">
      <n-text style="overflow-wrap: anywhere; user-select: text">{{ savedFile }}</n-text>
    </n-card>

    <n-card v-if="logs.length > 0" size="small" :title="$t('collector.logHeading')">
      <div class="log-box">
        <div v-for="(line, index) in logs" :key="index" class="log-line">{{ line }}</div>
      </div>
    </n-card>
  </n-flex>
</template>

<style scoped>
.steps {
  margin: 0;
  padding-left: 20px;
  line-height: 1.8;
}

.log-box {
  max-height: 220px;
  overflow-y: auto;
  font-size: 12px;
  line-height: 1.7;
}

.log-line {
  color: var(--n-text-color-3, #909399);
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
}
</style>
