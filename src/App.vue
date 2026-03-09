<script setup lang="ts">
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { exit } from "@tauri-apps/plugin-process";
import IconMdiHome from "~icons/mdi/home";
import IconMdiDownload from "~icons/mdi/download";
import IconMdiToolbox from "~icons/mdi/toolbox";
import type { Component } from "vue";
import { useI18n } from "vue-i18n";
import { useSettingStore } from "@/stores/setting";
import { useDownloadStore } from "@/stores/download";
import { localeEntries } from "@/locales";

const { t } = useI18n();
const router = useRouter();
const route = useRoute();
const settingStore = useSettingStore();
const downloadStore = useDownloadStore();

/** 同步托盘菜单语言 */
const syncTrayMenu = () => {
  invoke("update_tray_menu", {
    showLabel: t("tray.show"),
    quitLabel: t("tray.quit"),
  });
};

watch(() => settingStore.locale, syncTrayMenu);

/** 处理退出请求，有下载任务时弹出确认框 */
const handleQuitRequest = () => {
  if (downloadStore.activeCount > 0) {
    window.$dialog.warning({
      title: t("tray.quitConfirmTitle"),
      content: t("tray.quitConfirmContent"),
      positiveText: t("common.cancel"),
      negativeText: t("tray.quit"),
      onNegativeClick: () => exit(0),
    });
  } else {
    exit(0);
  }
};

const localeOptions = localeEntries.map((e) => ({ label: `${e.flag} ${e.label}`, value: e.code }));

const currentRoute = computed(() => {
  const name = (route.name as string) ?? "";
  if (name === "detail") return "home";
  if (name.startsWith("toolbox")) return "toolbox";
  return name;
});

const navItems: { key: string; icon: Component; labelKey: string }[] = [
  { key: "home", icon: IconMdiHome, labelKey: "nav.home" },
  { key: "downloads", icon: IconMdiDownload, labelKey: "nav.downloads" },
  { key: "toolbox", icon: IconMdiToolbox, labelKey: "nav.toolbox" },
];

const win = getCurrentWindow();

// 关闭窗口时的行为
win.onCloseRequested(async (event) => {
  if (settingStore.closeToTray) {
    event.preventDefault();
    await win.hide();
  } else {
    event.preventDefault();
    handleQuitRequest();
  }
});

// 监听托盘退出请求
listen("tray-quit-requested", () => handleQuitRequest());

onMounted(() => {
  win.show();
  syncTrayMenu();
});
</script>

<template>
  <Provider>
    <CookieModal />
    <UpdateModal />
    <SetupModal />
    <n-layout style="height: 100vh">
      <n-layout-header bordered class="app-header">
        <div class="header-side">
          <div class="logo" @click="router.push({ name: 'home' })">
            <icon-mdi-youtube />
            <span class="logo-text">GUI</span>
          </div>
        </div>
        <div class="header-nav">
          <n-button
            v-for="item in navItems"
            :key="item.key"
            :quaternary="currentRoute !== item.key"
            :type="currentRoute === item.key ? 'primary' : 'default'"
            :secondary="currentRoute === item.key"
            :focusable="false"
            round
            @click="router.push({ name: item.key })"
          >
            <template #icon>
              <n-icon>
                <component :is="item.icon" />
              </n-icon>
            </template>
            <span class="nav-label" :class="{ expanded: currentRoute === item.key }">
              {{ $t(item.labelKey) }}
            </span>
          </n-button>
        </div>
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
          <n-popselect v-model:value="settingStore.locale" :options="localeOptions" trigger="click">
            <n-button :focusable="false" quaternary circle>
              <template #icon>
                <n-icon>
                  <icon-mdi-translate />
                </n-icon>
              </template>
            </n-button>
          </n-popselect>
          <n-button
            :type="currentRoute === 'settings' ? 'primary' : 'default'"
            :secondary="currentRoute === 'settings'"
            :quaternary="currentRoute !== 'settings'"
            :focusable="false"
            circle
            @click="router.push({ name: 'settings' })"
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
        content-style="padding: 16px; display: flex; flex-direction: column; min-height: 100%;"
        :native-scrollbar="false"
      >
        <div style="flex: 1">
          <router-view v-slot="{ Component: RouteComponent }">
            <Transition name="fade-slide" mode="out-in">
              <component :is="RouteComponent" />
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
              YDL GUI
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
        max-width: 80px;
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
