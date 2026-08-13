<script setup lang="ts">
defineProps<{
  estimatedSizeText: string;
}>();

const emit = defineEmits<{
  download: [];
}>();
</script>

<template>
  <div class="bottom-bar">
    <n-card size="small" class="bottom-card">
      <n-flex align="center" justify="space-between">
        <n-flex align="center" :size="4" class="size-info">
          <icon-mdi-file-outline />
          <n-text depth="3">{{ $t("detail.estimatedSize") }}</n-text>
          <n-text strong>{{ estimatedSizeText }}</n-text>
        </n-flex>
        <n-button type="primary" round @click="emit('download')">
          <template #icon>
            <n-icon>
              <icon-mdi-download />
            </n-icon>
          </template>
          {{ $t("detail.downloadNow") }}
        </n-button>
      </n-flex>
    </n-card>
  </div>
</template>

<style scoped lang="scss">
.bottom-bar {
  position: fixed;
  bottom: 32px;
  left: 0;
  right: 0;
  padding: 16px 24px;
  z-index: 10;
  opacity: 0;
  animation: slide-up 0.3s ease-in-out forwards;
  animation-delay: 0.1s;

  .bottom-card {
    max-width: 100%;
    box-shadow: 0 -2px 12px rgba(0, 0, 0, 0.08);
  }

  .size-info {
    font-size: 13px;
    font-variant-numeric: tabular-nums;
  }
}

@keyframes slide-up {
  from {
    transform: translateY(100%);
    opacity: 0;
  }
  to {
    transform: translateY(0);
    opacity: 1;
  }
}
</style>
