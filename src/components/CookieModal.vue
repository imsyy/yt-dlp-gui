<script setup lang="ts">
import { useStatusStore } from "@/stores/status";

const statusStore = useStatusStore();
</script>

<template>
  <n-modal
    v-model:show="statusStore.showCookieModal"
    preset="card"
    :auto-focus="false"
    :title="$t('cookie.settings')"
    size="small"
    :bordered="false"
    content-scrollable
    :style="{ width: '480px', maxHeight: '80vh' }"
  >
    <CookieCard plain />
  </n-modal>
</template>
