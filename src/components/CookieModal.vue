<script setup lang="ts">
import { useStatusStore } from "@/stores/status";

const statusStore = useStatusStore();
</script>

<template>
  <n-modal
    v-model:show="statusStore.showCookieModal"
    preset="card"
    :title="$t('cookie.settings')"
    size="small"
    :bordered="false"
    :style="{ width: '480px' }"
  >
    <CookieCard />
  </n-modal>
</template>
