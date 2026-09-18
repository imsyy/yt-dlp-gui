<script setup lang="ts">
defineProps<{
  loading: boolean;
  disabled: boolean;
}>();

const emit = defineEmits<{ fetch: []; stop: [] }>();
</script>

<template>
  <n-flex :size="8" :wrap="false">
    <n-button
      type="primary"
      style="flex: 1"
      :loading="loading"
      :disabled="disabled || loading"
      @click="emit('fetch')"
    >
      <template #icon>
        <slot name="icon" />
      </template>
      <slot />
    </n-button>
    <n-button type="error" secondary :disabled="!loading" @click="emit('stop')">
      <template #icon>
        <n-icon><icon-mdi-stop /></n-icon>
      </template>
      {{ $t("toolbox.stopTask") }}
    </n-button>
  </n-flex>
</template>
