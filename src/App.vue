<script setup lang="ts">
import IconMdiHome from "~icons/mdi/home";
import IconMdiDownload from "~icons/mdi/download";
import type { Component } from "vue";

const router = useRouter();
const route = useRoute();

const currentRoute = computed(() => route.name as string);

function navigateTo(name: string) {
  router.push({ name });
}

const navItems: { key: string; icon: Component; label: string }[] = [
  { key: "home", icon: IconMdiHome, label: "首页" },
  { key: "downloads", icon: IconMdiDownload, label: "下载" },
];
</script>

<template>
  <Provider>
    <n-layout style="height: 100vh">
      <n-layout-header bordered class="app-header">
        <!-- 左侧 Logo -->
        <div class="header-side">
          <div class="logo" @click="navigateTo('home')">
            <icon-mdi-youtube />
            <span class="logo-text">GUI</span>
          </div>
        </div>

        <!-- 中间导航 -->
        <div class="header-nav">
          <n-button
            v-for="item in navItems"
            :key="item.key"
            :quaternary="currentRoute !== item.key"
            :type="currentRoute === item.key ? 'primary' : 'default'"
            :secondary="currentRoute === item.key"
            :focusable="false"
            round
            @click="navigateTo(item.key)"
          >
            <template #icon>
              <n-icon>
                <component :is="item.icon" />
              </n-icon>
            </template>
            <span
              class="nav-label"
              :class="{ expanded: currentRoute === item.key }"
            >
              {{ item.label }}
            </span>
          </n-button>
        </div>

        <!-- 右侧按钮 -->
        <div class="header-side header-side-right">
          <n-button
            :focusable="false"
            quaternary
            circle
            tag="a"
            href="https://github.com/imsyy/yt-dlp-gui"
            target="_blank"
          >
            <template #icon>
              <n-icon>
                <icon-mdi-github />
              </n-icon>
            </template>
          </n-button>
          <n-button
            :type="currentRoute === 'settings' ? 'primary' : 'default'"
            :secondary="currentRoute === 'settings'"
            :quaternary="currentRoute !== 'settings'"
            :focusable="false"
            circle
            @click="navigateTo('settings')"
          >
            <template #icon>
              <n-icon>
                <icon-mdi-cog />
              </n-icon>
            </template>
          </n-button>
        </div>
      </n-layout-header>

      <n-layout
        position="absolute"
        style="top: 56px"
        content-style="padding: 24px; display: flex; flex-direction: column; min-height: 100%;"
        :native-scrollbar="false"
      >
        <div style="flex: 1">
          <router-view v-slot="{ Component }">
            <Transition name="fade-slide" mode="out-in">
              <component :is="Component" />
            </Transition>
          </router-view>
        </div>
        <n-flex justify="center" align="center" :size="4" class="app-footer">
          <n-text depth="3" style="font-size: 12px">
            © {{ new Date().getFullYear() }}
            <n-button
              text
              tag="a"
              href="https://github.com/imsyy"
              target="_blank"
              size="tiny"
              style="font-size: 12px"
            >
              imsyy
            </n-button>
            ·
            <n-button
              text
              tag="a"
              href="https://github.com/imsyy/yt-dlp-gui"
              target="_blank"
              size="tiny"
              style="font-size: 12px"
            >
              yt-dlp-gui
            </n-button>
          </n-text>
        </n-flex>
      </n-layout>
    </n-layout>
  </Provider>
</template>

<style scoped lang="scss">
.app-header {
  height: 56px;
  display: flex;
  align-items: center;
  padding: 0 16px;

  .header-side {
    width: 120px;
    flex-shrink: 0;
    display: flex;
    align-items: center;

    &.header-side-right {
      justify-content: flex-end;
      gap: 4px;
    }
  }

  .logo {
    display: flex;
    align-items: center;
    gap: 6px;
    user-select: none;
    cursor: pointer;

    svg {
      font-size: 28px;
      transition: color 0.3s;
    }

    .logo-text {
      font-weight: 700;
      font-size: 16px;
      letter-spacing: 0.5px;
    }

    &:hover {
      svg {
        color: #ff0033;
      }
    }
  }

  .header-nav {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 4px;

    :deep(.n-button) {
      .n-button__content {
        transition:
          max-width 0.2s ease,
          opacity 0.2s ease;
      }

      .n-button__icon {
        margin-right: 0;
      }

      &:not(.n-button--color) .n-button__icon {
        margin-left: 0;
      }
    }

    .nav-label {
      display: inline-block;
      max-width: 0;
      opacity: 0;
      overflow: hidden;
      transition:
        max-width 0.2s ease,
        opacity 0.2s ease,
        margin 0.2s ease;
      margin-left: 0;

      &.expanded {
        max-width: 40px;
        opacity: 1;
        margin-left: 4px;
      }
    }
  }
}

.app-footer {
  padding: 24px 0 4px;
  flex-shrink: 0;
}
</style>
