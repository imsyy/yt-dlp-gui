<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { isValidUrl } from "@/utils/validate";
import { useSettingStore } from "@/stores/setting";
import { useVideoStore } from "@/stores/video";
import type { ThumbnailInfo, VideoInfo } from "@/types";

const settingStore = useSettingStore();
const videoStore = useVideoStore();
const toolUrl = inject<Ref<string>>("toolUrl")!;

const loading = ref(false);
const thumbnails = ref<ThumbnailInfo[]>([]);
const videoTitle = ref("");
const savingId = ref<string | null>(null);

const urlValid = computed(() => isValidUrl(toolUrl.value.trim()));

/** 获取缩略图文件扩展名 */
const getExtFromUrl = (url: string): string => {
  try {
    const pathname = new URL(url).pathname;
    const ext = pathname.split(".").pop()?.toLowerCase();
    if (ext && ["jpg", "jpeg", "png", "webp"].includes(ext)) return ext;
  } catch {
    // ignore
  }
  return "jpg";
};

/** 获取分辨率显示文本 */
const getResolutionLabel = (t: ThumbnailInfo): string => {
  if (t.width && t.height) return `${t.width} x ${t.height}`;
  if (t.resolution) return t.resolution;
  return "未知";
};

/** 获取视频信息并提取封面列表 */
const handleFetch = async () => {
  loading.value = true;
  thumbnails.value = [];
  videoTitle.value = "";
  try {
    const cookieFile = await videoStore.getCookieFile();
    const info = await invoke<VideoInfo>("tool_fetch_thumbnails", {
      url: toolUrl.value.trim(),
      cookieFile,
      proxy: settingStore.proxy || null,
    });
    videoTitle.value = info.title || "";
    const list = info.thumbnails || [];
    // 优先保留有明确尺寸的项，按分辨率去重
    const withSize = list.filter((t) => t.url && t.width && t.height);
    const seen = new Set<string>();
    const deduped = withSize.filter((t) => {
      const key = `${t.width}x${t.height}`;
      if (seen.has(key)) return false;
      seen.add(key);
      return true;
    });
    if (deduped.length > 0) {
      thumbnails.value = deduped.sort(
        (a, b) => (b.width || 0) * (b.height || 0) - (a.width || 0) * (a.height || 0),
      );
    } else if (info.thumbnail) {
      // 没有带尺寸的项，使用主封面 URL
      thumbnails.value = [{ url: info.thumbnail, id: "default" }];
    } else {
      window.$message.warning("未找到封面图片");
    }
  } catch (e: unknown) {
    window.$message.error(`获取失败: ${e}`);
  } finally {
    loading.value = false;
  }
};

/** 另存为 */
const handleSave = async (thumb: ThumbnailInfo) => {
  const ext = getExtFromUrl(thumb.url);
  const defaultName = videoTitle.value
    ? `${videoTitle.value.slice(0, 200)}.${ext}`
    : `thumbnail.${ext}`;

  const filePath = await save({
    title: "保存封面图片",
    defaultPath: defaultName,
    filters: [{ name: "图片文件", extensions: [ext, "jpg", "png", "webp"] }],
  });
  if (!filePath) return;

  const id = thumb.id || thumb.url;
  savingId.value = id;
  try {
    await invoke("tool_save_thumbnail", {
      url: thumb.url,
      filePath,
      proxy: settingStore.proxy || null,
    });
    window.$message.success("封面已保存");
  } catch (e: unknown) {
    window.$message.error(`保存失败: ${e}`);
  } finally {
    savingId.value = null;
  }
};
</script>

<template>
  <n-flex vertical :size="12">
    <n-flex align="center" :size="8">
      <n-button strong secondary size="small" @click="$router.back()">
        <template #icon>
          <n-icon><icon-mdi-arrow-left /></n-icon>
        </template>
        返回
      </n-button>
      <n-text strong style="font-size: 15px">下载视频封面</n-text>
    </n-flex>

    <n-card size="small">
      <n-flex vertical :size="12">
        <n-text depth="3" style="font-size: 13px">
          获取视频的全部封面图片，选择清晰度后另存为本地文件
        </n-text>
        <n-button
          type="primary"
          :loading="loading"
          :disabled="!urlValid || loading"
          @click="handleFetch"
        >
          <template #icon>
            <n-icon><icon-mdi-image-search /></n-icon>
          </template>
          获取封面列表
        </n-button>
      </n-flex>
    </n-card>

    <!-- 封面列表 -->
    <n-card v-if="thumbnails.length" size="small" :title="`共 ${thumbnails.length} 张封面`">
      <n-list hoverable clickable bordered>
        <n-list-item v-for="thumb in thumbnails" :key="thumb.id || thumb.url">
          <n-flex align="center" :size="12" :wrap="false">
            <n-image
              :src="thumb.url"
              lazy
              preview-disabled
              width="120"
              height="68"
              object-fit="cover"
              style="border-radius: 4px; flex-shrink: 0"
            />
            <n-flex align="center" justify="space-between" style="flex: 1; min-width: 0">
              <n-text style="font-size: 13px">{{ getResolutionLabel(thumb) }}</n-text>
              <n-button
                size="small"
                :loading="savingId === (thumb.id || thumb.url)"
                @click="handleSave(thumb)"
              >
                <template #icon>
                  <n-icon><icon-mdi-content-save-outline /></n-icon>
                </template>
                另存为
              </n-button>
            </n-flex>
          </n-flex>
        </n-list-item>
      </n-list>
    </n-card>
  </n-flex>
</template>
