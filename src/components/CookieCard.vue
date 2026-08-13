<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { readText } from "@tauri-apps/plugin-clipboard-manager";
import { useSettingStore } from "@/stores/setting";
import { useI18n } from "vue-i18n";

const { t } = useI18n();
const settingStore = useSettingStore();

const cookieModeOptions = computed(() => [
  { label: t("cookie.manual"), value: "text" },
  { label: t("cookie.file"), value: "file" },
  { label: t("cookie.browser"), value: "browser" },
]);

const platform = ref("");
const browserOptions = computed(() => [
  { label: "Chrome", value: "chrome" },
  { label: "Edge", value: "edge" },
  { label: "Firefox", value: "firefox" },
  { label: "Brave", value: "brave" },
  { label: "Opera", value: "opera" },
  { label: "Vivaldi", value: "vivaldi" },
  { label: "Chromium", value: "chromium" },
  { label: "Whale", value: "whale" },
  ...(platform.value === "macos" ? [{ label: "Safari", value: "safari" }] : []),
]);

onMounted(async () => {
  platform.value = await invoke<string>("get_platform");
  if (!browserOptions.value.some((option) => option.value === settingStore.cookieBrowser)) {
    settingStore.cookieBrowser = "chrome";
  }
});

/** 从剪贴板粘贴 Cookie 文本 */
const handlePasteCookie = async () => {
  try {
    const text = await readText();
    const trimmed = text.trim();
    if (!trimmed) {
      window.$message.warning(t("clipboard.empty"));
      return;
    }
    settingStore.cookieText = trimmed;
    window.$message.success(t("clipboard.pasteSuccess"));
  } catch {
    window.$message.warning(t("clipboard.readFailed"));
  }
};

/** 保存 Cookie 文本到应用数据目录 */
const handleSaveCookieText = async () => {
  if (!settingStore.cookieText.trim()) {
    window.$message.warning(t("cookie.emptyContent"));
    return;
  }
  try {
    await invoke<string>("save_cookie_text", {
      text: settingStore.cookieText,
    });
    window.$message.success(t("cookie.savedTo"));
  } catch (e: unknown) {
    const error = String(e);
    if (/err_invalid_cookie|err_empty_cookie/.test(error)) {
      window.$message.error(t("cookie.invalidFormat"));
    } else {
      window.$message.error(t("common.saveFailed", { e }));
    }
  }
};

/** 选择 Cookie 文件 */
const handleSelectFile = async () => {
  const selected = await open({
    multiple: false,
    title: t("cookie.selectFile"),
    filters: [
      { name: t("cookie.cookieFiles"), extensions: ["txt", "cookie", "cookies"] },
      { name: t("cookie.allFiles"), extensions: ["*"] },
    ],
  });
  if (selected) {
    settingStore.cookieFile = selected as string;
  }
};
</script>

<template>
  <n-card title="Cookie" size="small">
    <n-flex vertical :size="12">
      <n-text depth="3" style="font-size: 13px">
        {{ $t("cookie.desc") }}
      </n-text>

      <n-radio-group v-model:value="settingStore.cookieMode" size="small">
        <n-radio-button v-for="opt in cookieModeOptions" :key="opt.value" :value="opt.value">
          {{ opt.label }}
        </n-radio-button>
      </n-radio-group>

      <template v-if="settingStore.cookieMode === 'text'">
        <n-input
          v-model:value="settingStore.cookieText"
          type="textarea"
          :placeholder="$t('cookie.textPlaceholder')"
          :autosize="{ minRows: 3, maxRows: 8 }"
          size="small"
        />
        <n-flex justify="end" :size="8">
          <n-button size="small" @click="handlePasteCookie">
            <template #icon>
              <n-icon>
                <icon-mdi-content-paste />
              </n-icon>
            </template>
            {{ $t("home.pasteFromClipboard") }}
          </n-button>
          <n-button
            size="small"
            :disabled="!settingStore.cookieText.trim()"
            @click="handleSaveCookieText"
          >
            <template #icon>
              <n-icon>
                <icon-mdi-content-save-outline />
              </n-icon>
            </template>
            {{ $t("common.save") }}
          </n-button>
        </n-flex>
      </template>

      <template v-if="settingStore.cookieMode === 'file'">
        <n-flex align="center" :size="8">
          <n-input
            :value="settingStore.cookieFile"
            :placeholder="$t('cookie.noFileSelected')"
            size="small"
            readonly
            style="flex: 1"
          />
          <n-button size="small" @click="handleSelectFile">
            <template #icon>
              <n-icon>
                <icon-mdi-file-search-outline />
              </n-icon>
            </template>
            {{ $t("common.select") }}
          </n-button>
        </n-flex>
      </template>

      <template v-if="settingStore.cookieMode === 'browser'">
        <n-alert type="warning" :bordered="false">
          {{ $t("cookie.browserDesc") }}
        </n-alert>
        <n-select
          v-model:value="settingStore.cookieBrowser"
          :options="browserOptions"
          size="small"
        />
      </template>
    </n-flex>
  </n-card>
</template>
