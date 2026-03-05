<template>
  <div class="home-page">
    <n-result status="info" title="yt-dlp GUI" description="在此处粘贴视频链接以开始下载">
      <template #footer>
        <n-input-group>
          <n-input
            v-model:value="videoUrl"
            placeholder="请输入视频链接..."
            size="large"
            clearable
          />
          <n-button type="primary" size="large" :disabled="!videoUrl">
            解析
          </n-button>
        </n-input-group>
      </template>
    </n-result>
  </div>
</template>

<script setup lang="ts">
const videoUrl = ref("");
</script>

<style scoped lang="scss">
.home-page {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
}
</style>
