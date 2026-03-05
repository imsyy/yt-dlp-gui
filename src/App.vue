<script setup lang="ts">
import { Icon } from "@iconify/vue";

const router = useRouter();
const route = useRoute();

const currentRoute = computed(() => route.name as string);

function navigateTo(name: string) {
  router.push({ name });
}

const navItems = [
  { key: "home", icon: "mdi:home", label: "首页" },
  { key: "downloads", icon: "mdi:download", label: "下载" },
];
</script>

<template>
  <Provider>
    <n-layout style="height: 100vh">
      <n-layout-header bordered class="app-header">
        <!-- 左侧 Logo -->
        <div class="header-side">
          <div class="logo" @click="navigateTo('home')">
            <Icon icon="mdi:youtube" />
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
                <Icon :icon="item.icon" />
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
            href="https://github.com"
            target="_blank"
          >
            <template #icon>
              <n-icon>
                <Icon icon="mdi:github" />
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
                <Icon icon="mdi:cog" />
              </n-icon>
            </template>
          </n-button>
        </div>
      </n-layout-header>

      <n-layout
        position="absolute"
        style="top: 56px"
        content-style="padding: 24px;"
        :native-scrollbar="false"
      >
        <router-view v-slot="{ Component }">
          <Transition name="fade-slide" mode="out-in">
            <component :is="Component" />
          </Transition>
        </router-view>
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

    .iconify {
      font-size: 28px;
      transition: color 0.3s;
    }

    .logo-text {
      font-weight: 700;
      font-size: 16px;
      letter-spacing: 0.5px;
    }

    &:hover {
      .iconify {
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

// 路由切换动画
.fade-slide-enter-active,
.fade-slide-leave-active {
  transition: all 0.15s ease;
}

.fade-slide-enter-from {
  opacity: 0;
  transform: translateX(8px);
}

.fade-slide-leave-to {
  opacity: 0;
  transform: translateX(-8px);
}
</style>
