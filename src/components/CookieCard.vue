<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useSettingStore } from "@/stores/setting";

const settingStore = useSettingStore();

const cookieModeOptions = [
  { label: "不使用", value: "none" },
  { label: "手动输入", value: "text" },
  { label: "Cookie 文件", value: "file" },
];

/** 保存 Cookie 文本到应用数据目录 */
const handleSaveCookieText = async () => {
  if (!settingStore.cookieText.trim()) {
    window.$message.warning("Cookie 内容为空");
    return;
  }
  try {
    const path = await invoke<string>("save_cookie_text", {
      text: settingStore.cookieText,
    });
    window.$message.success(`Cookie 已保存至 ${path}`);
  } catch (e: any) {
    window.$message.error(`保存失败: ${e}`);
  }
};

/** 选择 Cookie 文件 */
const handleSelectFile = async () => {
  const selected = await open({
    multiple: false,
    title: "选择 Cookie 文件",
    filters: [
      { name: "Cookie 文件", extensions: ["txt", "cookie", "cookies"] },
      { name: "所有文件", extensions: ["*"] },
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
        部分视频需要登录后才能下载，可通过 Cookie 实现身份验证
      </n-text>

      <n-radio-group v-model:value="settingStore.cookieMode" size="small">
        <n-radio-button
          v-for="opt in cookieModeOptions"
          :key="opt.value"
          :value="opt.value"
        >
          {{ opt.label }}
        </n-radio-button>
      </n-radio-group>

      <!-- 手动输入模式 -->
      <template v-if="settingStore.cookieMode === 'text'">
        <n-input
          v-model:value="settingStore.cookieText"
          type="textarea"
          placeholder="粘贴 Netscape 格式的 Cookie 内容..."
          :autosize="{ minRows: 3, maxRows: 8 }"
          size="small"
        />
        <n-flex justify="end">
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
            保存
          </n-button>
        </n-flex>
      </template>

      <!-- 文件模式 -->
      <template v-if="settingStore.cookieMode === 'file'">
        <n-flex align="center" :size="8">
          <n-input
            :value="settingStore.cookieFile"
            placeholder="未选择文件"
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
            选择
          </n-button>
        </n-flex>
      </template>
    </n-flex>
  </n-card>
</template>
