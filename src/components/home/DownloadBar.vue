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
          <n-text depth="3">预估大小:</n-text>
          <n-text strong>{{ estimatedSizeText }}</n-text>
        </n-flex>
        <n-button type="primary" round @click="emit('download')">
          <template #icon>
            <n-icon>
              <icon-mdi-download />
            </n-icon>
          </template>
          立即下载
        </n-button>
      </n-flex>
    </n-card>
  </div>
</template>

<style scoped lang="scss">
.bottom-bar {
  position: fixed;
  bottom: 0;
  left: 0;
  right: 0;
  padding: 16px 24px;
  z-index: 10;

  .bottom-card {
    max-width: 100%;
    box-shadow: 0 -2px 12px rgba(0, 0, 0, 0.08);
  }

  .size-info {
    font-size: 13px;
  }
}
</style>
