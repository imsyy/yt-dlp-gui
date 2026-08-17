<script setup lang="ts">
import type { HomeDownloadBehavior } from "@/types";

const behavior = defineModel<HomeDownloadBehavior>({ required: true });
defineEmits<{ settings: [] }>();
</script>

<template>
  <div class="behavior-controls">
    <n-tabs
      v-model:value="behavior"
      type="segment"
      size="small"
      animated
      class="behavior-tabs"
    >
      <n-tab-pane name="pending">
        <template #tab>
          <n-tooltip>
            <template #trigger>
              <n-icon size="16" :aria-label="$t('home.pendingBehavior')">
                <icon-mdi-playlist-edit />
              </n-icon>
            </template>
            {{ $t("home.pendingBehavior") }}
          </n-tooltip>
        </template>
      </n-tab-pane>
      <n-tab-pane name="quick">
        <template #tab>
          <n-tooltip>
            <template #trigger>
              <n-icon size="16" :aria-label="$t('home.quickBehavior')">
                <icon-mdi-lightning-bolt />
              </n-icon>
            </template>
            {{ $t("home.quickBehavior") }}
          </n-tooltip>
        </template>
      </n-tab-pane>
    </n-tabs>

    <Transition name="settings-pop">
      <span v-if="behavior === 'quick'" class="settings-action">
        <n-tooltip>
          <template #trigger>
            <n-button
              size="small"
              circle
              secondary
              :aria-label="$t('home.quickSettings')"
              @click="$emit('settings')"
            >
              <template #icon>
                <n-icon size="16"><icon-mdi-cog-outline /></n-icon>
              </template>
            </n-button>
          </template>
          {{ $t("home.quickSettings") }}
        </n-tooltip>
      </span>
    </Transition>
  </div>
</template>

<style scoped>
.behavior-controls {
  display: flex;
  align-items: center;
  gap: 6px;
}

.behavior-tabs {
  width: 76px;
}

.behavior-tabs :deep(.n-tabs-pane-wrapper) {
  display: none;
}

.settings-action {
  display: inline-flex;
  transform-origin: left center;
}

.settings-pop-enter-active,
.settings-pop-leave-active {
  transition-property: opacity, transform, filter;
  transition-duration: 160ms;
  transition-timing-function: cubic-bezier(0.2, 0, 0, 1);
}

.settings-pop-enter-from,
.settings-pop-leave-to {
  opacity: 0;
  transform: scale(0.25);
  filter: blur(4px);
}
</style>
