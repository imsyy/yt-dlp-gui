<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { useSettingStore } from "@/stores/setting";
import { useI18n } from "vue-i18n";

const { t } = useI18n();
const settingStore = useSettingStore();

const scheme = ref<"http" | "https" | "socks5">("http");
const host = ref("");
const port = ref<number | null>(null);
const username = ref("");
const password = ref("");

const schemeOptions = computed(() => [
  { label: "HTTP", value: "http" },
  { label: "HTTPS", value: "https" },
  { label: "SOCKS5", value: "socks5" },
]);

/** 解析已保存的代理 URL 到表单字段 */
const parseProxy = (url: string) => {
  const trimmed = url?.trim();
  if (!trimmed) return;
  const match = trimmed.match(
    /^(https?|socks5):\/\/(?:(.+?):(.+?)@)?(\[[^\]]+\]|[^:]+)(?::(\d+))?$/i,
  );
  if (!match) return;
  scheme.value = (match[1] || "http").toLowerCase() as typeof scheme.value;
  username.value = decodeURIComponent(match[2] || "");
  password.value = decodeURIComponent(match[3] || "");
  host.value = match[4] || "";
  port.value = match[5] ? Number(match[5]) : null;
};

/** 将当前表单字段组合为代理 URL */
const composeProxy = (): string => {
  const h = host.value.trim();
  if (!h) return "";
  let url = `${scheme.value}://`;
  if (username.value || password.value) {
    url += `${encodeURIComponent(username.value)}:${encodeURIComponent(password.value)}@`;
  }
  url += h;
  if (port.value) url += `:${port.value}`;
  return url;
};

onMounted(() => {
  parseProxy(settingStore.proxy);
});

/** 测试代理连通性 */
const testing = ref(false);
const handleTest = async () => {
  const url = composeProxy();
  if (!url) {
    window.$message.warning(t("settings.proxyInvalid"));
    return;
  }
  testing.value = true;
  try {
    await invoke("test_proxy", { proxy: url });
    window.$message.success(t("settings.proxyTestSuccess"));
  } catch (e: unknown) {
    window.$message.error(t("settings.proxyTestFailed", { e }));
  } finally {
    testing.value = false;
  }
};

/** 应用代理设置 */
const handleApply = () => {
  settingStore.proxy = composeProxy();
  window.$message.success(t("settings.proxyApplied"));
};
</script>

<template>
  <n-card :title="$t('settings.proxy')" size="small">
    <n-flex vertical :size="12">
      <n-text depth="3" style="font-size: 13px">
        {{ $t("settings.proxyDesc") }}
      </n-text>
      <div class="info-list">
        <div class="info-row">
          <span class="info-label">{{ $t("settings.proxyScheme") }}</span>
          <n-select
            v-model:value="scheme"
            :options="schemeOptions"
            size="small"
            style="width: 120px"
          />
        </div>
        <div class="info-row">
          <span class="info-label">{{ $t("settings.proxyHost") }}</span>
          <n-input
            v-model:value="host"
            :placeholder="$t('settings.proxyHostPlaceholder')"
            size="small"
            clearable
            style="width: 220px"
          />
        </div>
        <div class="info-row">
          <span class="info-label">{{ $t("settings.proxyPort") }}</span>
          <n-input-number
            v-model:value="port"
            :min="1"
            :max="65535"
            :placeholder="$t('settings.proxyPortPlaceholder')"
            size="small"
            clearable
            style="width: 120px"
          />
        </div>
        <div class="info-row">
          <span class="info-label">{{ $t("settings.proxyUsername") }}</span>
          <n-input
            v-model:value="username"
            :placeholder="$t('settings.proxyUsernamePlaceholder')"
            size="small"
            clearable
            style="width: 220px"
          />
        </div>
        <div class="info-row">
          <span class="info-label">{{ $t("settings.proxyPassword") }}</span>
          <n-input
            v-model:value="password"
            type="password"
            show-password-on="click"
            :placeholder="$t('settings.proxyPasswordPlaceholder')"
            size="small"
            clearable
            style="width: 220px"
          />
        </div>
      </div>
      <n-flex justify="end" :size="8">
        <n-button size="small" :loading="testing" :disabled="!host.trim()" @click="handleTest">
          <template #icon>
            <n-icon>
              <icon-mdi-sync />
            </n-icon>
          </template>
          {{ $t("settings.proxyTest") }}
        </n-button>
        <n-button size="small" type="primary" @click="handleApply">
          <template #icon>
            <n-icon>
              <icon-mdi-check />
            </n-icon>
          </template>
          {{ $t("settings.proxyApply") }}
        </n-button>
      </n-flex>
    </n-flex>
  </n-card>
</template>

<style scoped lang="scss">
.info-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.info-row {
  display: flex;
  align-items: center;
  font-size: 13px;
  min-height: 28px;

  &::before {
    order: 1;
    content: "";
    flex: 1;
    border-bottom: 1px dashed var(--n-border-color, #e0e0e6);
    margin: 0 8px;
    min-width: 20px;
  }

  > :last-child {
    order: 2;
    flex-shrink: 0;
  }
}

.info-label {
  flex-shrink: 0;
  order: 0;
}
</style>
