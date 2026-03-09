<script setup lang="ts">
import { useStatusStore } from "@/stores/status";

const statusStore = useStatusStore();
const router = useRouter();

const goToSettings = () => {
  statusStore.showYtdlpSetupModal = false;
  statusStore.showDenoSetupModal = false;
  router.push({ name: "settings" });
};
</script>

<template>
  <!-- yt-dlp 未安装 -->
  <n-modal
    v-model:show="statusStore.showYtdlpSetupModal"
    preset="card"
    :title="$t('setup.ytdlpTitle')"
    size="small"
    :bordered="false"
    :style="{ width: '460px' }"
  >
    <n-flex vertical :size="16">
      <n-alert type="error" :bordered="false">
        {{ $t('setup.ytdlpDesc') }}
      </n-alert>
      <n-text depth="3" style="font-size: 13px">
        {{ $t('setup.ytdlpHint') }}
      </n-text>
    </n-flex>
    <template #action>
      <n-flex justify="end">
        <n-button @click="statusStore.showYtdlpSetupModal = false">
          {{ $t('common.cancel') }}
        </n-button>
        <n-button type="primary" @click="goToSettings">
          {{ $t('setup.goToSettings') }}
        </n-button>
      </n-flex>
    </template>
  </n-modal>

  <!-- Deno 未安装 -->
  <n-modal
    v-model:show="statusStore.showDenoSetupModal"
    preset="card"
    :title="$t('setup.denoTitle')"
    size="small"
    :bordered="false"
    :style="{ width: '460px' }"
  >
    <n-flex vertical :size="16">
      <n-alert type="warning" :bordered="false">
        {{ $t('setup.denoDesc') }}
      </n-alert>
      <n-text depth="3" style="font-size: 13px">
        {{ $t('setup.denoHint') }}
      </n-text>
    </n-flex>
    <template #action>
      <n-flex justify="end">
        <n-button @click="statusStore.showDenoSetupModal = false">
          {{ $t('setup.later') }}
        </n-button>
        <n-button type="primary" @click="goToSettings">
          {{ $t('setup.goToSettings') }}
        </n-button>
      </n-flex>
    </template>
  </n-modal>
</template>
