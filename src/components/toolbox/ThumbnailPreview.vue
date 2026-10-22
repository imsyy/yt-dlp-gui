<script setup lang="ts">
import { useThumbnail } from "@/composables/useThumbnail";

const props = defineProps<{ src: string }>();
const { src: thumbnailSrc } = useThumbnail(() => props.src);
</script>

<template>
  <n-image :src="thumbnailSrc" />
</template>
