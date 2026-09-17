/**
 * Popup script for YDL GUI Helper.
 * 负责：多语言注入、标准/批量模式切换、多标签页勾选与状态管理、深链接发送与失败降级提示。
 */

import { sendToApp } from "./bridge.js";
import { createTranslator, getLanguage, setLanguage } from "./i18n.js";

let translate = (key, substitutions) => key;

/**
 * 校验 URL 是否为受支持的 HTTP/HTTPS 协议链接
 *
 * @param {string} url 目标地址
 * @returns {boolean} 是否为合法且支持的网络链接
 */
const isSupportedUrl = (url) => {
  if (!url) return false;
  try {
    return ["http:", "https:"].includes(new URL(url).protocol);
  } catch {
    return false;
  }
};

/**
 * 扫描并使用当前多语言字典填充所有带有 [data-i18n] 属性的 DOM 节点
 */
const applyI18n = () => {
  document.querySelectorAll("[data-i18n]").forEach((element) => {
    const key = element.getAttribute("data-i18n");
    const message = translate(key);
    if (message) element.textContent = message;
  });
};

/**
 * 更新标准模式下的顶部状态药丸胶囊展示
 *
 * @param {"ok" | "bad" | "muted"} kind 状态种类（成功/失败/静止）
 * @param {string} messageKey 多语言键名
 */
const setStatus = (kind, messageKey) => {
  const statusElement = document.getElementById("status");
  const textElement = document.getElementById("status-text");
  statusElement.classList.remove("pill-ok", "pill-bad", "pill-muted");
  statusElement.classList.add(
    kind === "ok" ? "pill-ok" : kind === "bad" ? "pill-bad" : "pill-muted",
  );
  textElement.textContent = translate(messageKey);
};

document.addEventListener("DOMContentLoaded", async () => {
  translate = await createTranslator();
  applyI18n();

  const languageSelect = document.getElementById("language");
  languageSelect.value = await getLanguage();
  languageSelect.addEventListener("change", async () => {
    await setLanguage(languageSelect.value);
    window.location.reload();
  });

  const standardBtn = document.getElementById("mode-standard-btn");
  const batchBtn = document.getElementById("mode-batch-btn");
  const panelStandard = document.getElementById("panel-standard");
  const panelBatch = document.getElementById("panel-batch");

  const sendBtn = document.getElementById("send-btn");
  const btnText = document.getElementById("btn-text");
  const urlPreview = document.getElementById("url-preview");
  const sendCookies = document.getElementById("send-cookies");
  const cookieRow = document.getElementById("cookie-row");
  const fallback = document.getElementById("fallback");

  const batchList = document.getElementById("batch-list");
  const batchEmpty = document.getElementById("batch-empty");
  const batchSelectAll = document.getElementById("batch-select-all");
  const batchDeselectAll = document.getElementById("batch-deselect-all");

  let currentMode = "standard";
  let activeTab = null;
  let validBatchTabs = [];

  // 1. 获取当前活动标签页（用于标准模式）
  const [currentTab] = await chrome.tabs.query({ active: true, currentWindow: true });
  activeTab = currentTab;
  const pageUrl = activeTab?.url || "";
  const isPageSupported = isSupportedUrl(pageUrl);

  if (!isPageSupported) {
    setStatus("bad", "popupUnsupported");
    urlPreview.hidden = true;
  } else {
    setStatus("ok", "popupSupported");
    urlPreview.hidden = false;
    urlPreview.textContent = pageUrl;
  }

  // 2. 获取当前窗口中所有有效标签页（用于批量模式）
  const allWindowTabs = await chrome.tabs.query({ currentWindow: true });
  validBatchTabs = allWindowTabs
    .filter((tabItem) => isSupportedUrl(tabItem.url))
    .map((tabItem) => ({
      id: tabItem.id,
      url: tabItem.url,
      title: tabItem.title || tabItem.url,
      favIconUrl: tabItem.favIconUrl,
      selected: true,
    }));

  /**
   * 渲染批量模式下的标签页多选列表
   */
  const renderBatchTabs = () => {
    batchList.innerHTML = "";
    if (validBatchTabs.length === 0) {
      batchEmpty.hidden = false;
      batchList.hidden = true;
      return;
    }
    batchEmpty.hidden = true;
    batchList.hidden = false;

    validBatchTabs.forEach((item) => {
      const row = document.createElement("label");
      row.className = "batch-item";

      const checkbox = document.createElement("input");
      checkbox.type = "checkbox";
      checkbox.checked = item.selected;
      checkbox.addEventListener("change", () => {
        item.selected = checkbox.checked;
        updateSendBtnState();
      });

      const icon = document.createElement("img");
      icon.className = "batch-item-icon";
      icon.src = item.favIconUrl || "icons/icon16.png";
      icon.alt = "";
      icon.addEventListener("error", () => {
        icon.src = "icons/icon16.png";
      });

      const infoContainer = document.createElement("div");
      infoContainer.className = "batch-item-info";

      const titleElement = document.createElement("div");
      titleElement.className = "batch-item-title";
      titleElement.textContent = item.title;

      const urlElement = document.createElement("div");
      urlElement.className = "batch-item-url";
      urlElement.textContent = item.url;

      infoContainer.appendChild(titleElement);
      infoContainer.appendChild(urlElement);

      row.appendChild(checkbox);
      row.appendChild(icon);
      row.appendChild(infoContainer);
      batchList.appendChild(row);
    });
  };

  renderBatchTabs();

  batchSelectAll.addEventListener("click", () => {
    validBatchTabs.forEach((tabItem) => (tabItem.selected = true));
    renderBatchTabs();
    updateSendBtnState();
  });

  batchDeselectAll.addEventListener("click", () => {
    validBatchTabs.forEach((tabItem) => (tabItem.selected = false));
    renderBatchTabs();
    updateSendBtnState();
  });

  /**
   * 根据当前所处模式及选中状态动态更新发送按钮文本与可点击状态
   */
  const updateSendBtnState = () => {
    if (currentMode === "standard") {
      sendBtn.disabled = !isPageSupported;
      btnText.textContent = translate("popupSendBtn");
      cookieRow.style.opacity = isPageSupported ? "1" : "0.55";
      sendCookies.disabled = !isPageSupported;
    } else {
      const selectedCount = validBatchTabs.filter((tabItem) => tabItem.selected).length;
      sendBtn.disabled = selectedCount === 0;
      btnText.textContent = `${translate("modeBatch")} (${selectedCount})`;
      cookieRow.style.opacity = selectedCount > 0 ? "1" : "0.55";
      sendCookies.disabled = selectedCount === 0;
    }
  };

  /**
   * 切换标准模式与批量模式视图
   *
   * @param {"standard" | "batch"} mode 目标模式
   */
  const switchMode = (mode) => {
    currentMode = mode;
    if (mode === "standard") {
      standardBtn.classList.add("active");
      batchBtn.classList.remove("active");
      panelStandard.hidden = false;
      panelBatch.hidden = true;
    } else {
      standardBtn.classList.remove("active");
      batchBtn.classList.add("active");
      panelStandard.hidden = true;
      panelBatch.hidden = false;
    }
    fallback.hidden = true;
    updateSendBtnState();
  };

  standardBtn.addEventListener("click", () => switchMode("standard"));
  batchBtn.addEventListener("click", () => switchMode("batch"));

  switchMode("standard");

  sendBtn.addEventListener("click", async () => {
    sendBtn.disabled = true;
    btnText.textContent = translate("popupSending");
    fallback.hidden = true;

    try {
      if (currentMode === "standard") {
        await sendToApp(pageUrl, {
          withCookies: sendCookies.checked,
          mode: "standard",
          tabId: activeTab?.id,
        });
      } else {
        const selectedTabs = validBatchTabs.filter((tabItem) => tabItem.selected);
        await sendToApp(selectedTabs.map((tabItem) => tabItem.url), {
          withCookies: sendCookies.checked,
          mode: "batch",
          items: selectedTabs.map((tabItem) => ({ url: tabItem.url, tabId: tabItem.id })),
        });
      }

      btnText.textContent = translate("popupSent");
      if (currentMode === "standard") {
        setStatus("ok", "popupSent");
      }
    } catch (error) {
      console.error("[YDL GUI] send failed:", error);
      btnText.textContent = translate("popupFailed");
      if (currentMode === "standard") {
        setStatus("bad", "popupFailed");
      }
      fallback.hidden = false;
      sendBtn.disabled = false;
    }
  });
});
