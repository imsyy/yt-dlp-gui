/**
 * Popup script for YDL GUI Helper.
 * 负责：多语言注入、标准/批量模式切换、当前标签页信息联动、多标签页勾选与状态管理、深链接/本地桥发送与恢复。
 */

import { sendToApp } from "./bridge.js";
import { createTranslator, getLanguage, setLanguage } from "./i18n.js";

let translate = (translationKey, substitutions) => translationKey;

/**
 * 校验 URL 是否为受支持的 HTTP/HTTPS 协议网络链接
 *
 * @param {string} candidateUrl 待检测的目标地址
 * @returns {boolean} 是否为合法且支持的网络链接
 */
const isSupportedUrl = (candidateUrl) => {
  if (!candidateUrl) return false;
  try {
    return ["http:", "https:"].includes(new URL(candidateUrl).protocol);
  } catch {
    return false;
  }
};

/**
 * 扫描并使用当前多语言字典填充所有带有 [data-i18n] 属性的 DOM 节点
 */
const applyI18n = () => {
  document.querySelectorAll("[data-i18n]").forEach((domElement) => {
    const translationKey = domElement.getAttribute("data-i18n");
    const localizedText = translate(translationKey);
    if (localizedText) domElement.textContent = localizedText;
  });
};

/**
 * 更新标准模式下的状态药丸胶囊展示
 *
 * @param {"ok" | "bad" | "muted"} pillKind 状态类型（成功/失败/中性）
 * @param {string} translationKey 多语言键名
 */
const setStatusPill = (pillKind, translationKey) => {
  const statusElement = document.getElementById("status");
  const textElement = document.getElementById("status-text");
  if (!statusElement || !textElement) return;

  statusElement.classList.remove("pill-ok", "pill-bad", "pill-muted");
  statusElement.classList.add(
    pillKind === "ok" ? "pill-ok" : pillKind === "bad" ? "pill-bad" : "pill-muted",
  );
  textElement.textContent = translate(translationKey);
};

/**
 * 健壮获取当前浏览器宿主窗口的活动标签页
 * 优先采用 lastFocusedWindow 查询，再回退至 currentWindow 和全局 active tab
 *
 * @returns {Promise<chrome.tabs.Tab | null>} 当前活动标签页对象
 */
const queryActiveTab = async () => {
  try {
    const focusedTabs = await chrome.tabs.query({ active: true, lastFocusedWindow: true });
    if (focusedTabs && focusedTabs.length > 0 && focusedTabs[0].url) {
      return focusedTabs[0];
    }
  } catch {}

  try {
    const currentTabs = await chrome.tabs.query({ active: true, currentWindow: true });
    if (currentTabs && currentTabs.length > 0 && currentTabs[0].url) {
      return currentTabs[0];
    }
  } catch {}

  try {
    const anyActiveTabs = await chrome.tabs.query({ active: true });
    if (anyActiveTabs && anyActiveTabs.length > 0) {
      return anyActiveTabs[0];
    }
  } catch {}

  return null;
};

/**
 * 健壮获取当前浏览器宿主窗口的所有标签页
 *
 * @returns {Promise<chrome.tabs.Tab[]>} 标签页列表
 */
const queryAllWindowTabs = async () => {
  try {
    const tabs = await chrome.tabs.query({ lastFocusedWindow: true });
    if (tabs && tabs.length > 0) return tabs;
  } catch {}

  try {
    const tabs = await chrome.tabs.query({ currentWindow: true });
    if (tabs && tabs.length > 0) return tabs;
  } catch {}

  return [];
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

  const tabIcon = document.getElementById("tab-icon");
  const tabTitle = document.getElementById("tab-title");
  const urlInput = document.getElementById("url-input");

  const sendBtn = document.getElementById("send-btn");
  const btnText = document.getElementById("btn-text");
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
  let isSubmitting = false;

  // 1. 初始化标准模式：获取当前活动标签页
  activeTab = await queryActiveTab();
  const initialUrl = activeTab?.url || "";
  const isInitialUrlSupported = isSupportedUrl(initialUrl);

  if (activeTab) {
    tabTitle.textContent = activeTab.title || activeTab.url || "Untitled";
    tabIcon.src = activeTab.favIconUrl || "icons/icon16.png";
    tabIcon.addEventListener("error", () => {
      tabIcon.src = "icons/icon16.png";
    });
  }

  if (isInitialUrlSupported) {
    urlInput.value = initialUrl;
    setStatusPill("ok", "popupSupported");
  } else {
    urlInput.value = "";
    urlInput.placeholder = "https://...";
    setStatusPill("bad", "popupUnsupported");
  }

  // 2. 初始化批量模式：获取当前窗口中的所有有效网页标签
  const rawWindowTabs = await queryAllWindowTabs();
  validBatchTabs = rawWindowTabs
    .filter((tabCandidate) => isSupportedUrl(tabCandidate.url))
    .map((tabCandidate) => ({
      id: tabCandidate.id,
      url: tabCandidate.url,
      title: tabCandidate.title || tabCandidate.url,
      favIconUrl: tabCandidate.favIconUrl,
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

    validBatchTabs.forEach((tabItem) => {
      const rowLabel = document.createElement("label");
      rowLabel.className = "batch-item";

      const checkbox = document.createElement("input");
      checkbox.type = "checkbox";
      checkbox.checked = tabItem.selected;
      checkbox.addEventListener("change", () => {
        tabItem.selected = checkbox.checked;
        updateSendBtnState();
      });

      const iconImg = document.createElement("img");
      iconImg.className = "batch-item-icon";
      iconImg.src = tabItem.favIconUrl || "icons/icon16.png";
      iconImg.alt = "";
      iconImg.addEventListener("error", () => {
        iconImg.src = "icons/icon16.png";
      });

      const infoContainer = document.createElement("div");
      infoContainer.className = "batch-item-info";

      const titleDiv = document.createElement("div");
      titleDiv.className = "batch-item-title";
      titleDiv.textContent = tabItem.title;

      const urlDiv = document.createElement("div");
      urlDiv.className = "batch-item-url";
      urlDiv.textContent = tabItem.url;

      infoContainer.appendChild(titleDiv);
      infoContainer.appendChild(urlDiv);

      rowLabel.appendChild(checkbox);
      rowLabel.appendChild(iconImg);
      rowLabel.appendChild(infoContainer);
      batchList.appendChild(rowLabel);
    });
  };

  renderBatchTabs();

  batchSelectAll.addEventListener("click", () => {
    validBatchTabs.forEach((tabCandidate) => (tabCandidate.selected = true));
    renderBatchTabs();
    updateSendBtnState();
  });

  batchDeselectAll.addEventListener("click", () => {
    validBatchTabs.forEach((tabCandidate) => (tabCandidate.selected = false));
    renderBatchTabs();
    updateSendBtnState();
  });

  /**
   * 根据当前模式及输入/选中状态动态更新发送按钮文本与可点击状态
   */
  const updateSendBtnState = () => {
    if (isSubmitting) return;

    if (currentMode === "standard") {
      const currentInputValue = urlInput.value.trim();
      const isInputSupported = isSupportedUrl(currentInputValue);

      sendBtn.disabled = !isInputSupported;
      btnText.textContent = translate("popupSendBtn");
      cookieRow.style.opacity = isInputSupported ? "1" : "0.55";
      sendCookies.disabled = !isInputSupported;

      if (isInputSupported) {
        setStatusPill("ok", "popupSupported");
      } else {
        setStatusPill("bad", "popupUnsupported");
      }
    } else {
      const selectedCount = validBatchTabs.filter((tabCandidate) => tabCandidate.selected).length;
      sendBtn.disabled = selectedCount === 0;
      btnText.textContent = translate("popupSendBatchBtn", { count: selectedCount });
      cookieRow.style.opacity = selectedCount > 0 ? "1" : "0.55";
      sendCookies.disabled = selectedCount === 0;
    }
  };

  // 监听标准模式输入框实时变化
  urlInput.addEventListener("input", () => {
    updateSendBtnState();
  });

  /**
   * 切换标准模式与批量模式视图
   *
   * @param {"standard" | "batch"} nextMode 目标模式
   */
  const switchMode = (nextMode) => {
    currentMode = nextMode;
    if (nextMode === "standard") {
      standardBtn.classList.add("active");
      batchBtn.classList.remove("active");
      panelStandard.hidden = false;
      panelStandard.style.display = "flex";
      panelBatch.hidden = true;
      panelBatch.style.display = "none";
    } else {
      standardBtn.classList.remove("active");
      batchBtn.classList.add("active");
      panelStandard.hidden = true;
      panelStandard.style.display = "none";
      panelBatch.hidden = false;
      panelBatch.style.display = "flex";
    }
    fallback.hidden = true;
    fallback.style.display = "none";
    updateSendBtnState();
  };

  standardBtn.addEventListener("click", () => switchMode("standard"));
  batchBtn.addEventListener("click", () => switchMode("batch"));

  // 初始化初始模式状态
  switchMode("standard");

  /**
   * 点击发送主按钮，根据当前模式将单条或批量链接及 Cookie 转发至客户端
   */
  sendBtn.addEventListener("click", async () => {
    if (isSubmitting) return;

    isSubmitting = true;
    sendBtn.disabled = true;
    btnText.textContent = translate("popupSending");
    fallback.hidden = true;

    try {
      if (currentMode === "standard") {
        const targetUrl = urlInput.value.trim();
        await sendToApp(targetUrl, {
          withCookies: sendCookies.checked,
          mode: "standard",
          tabId: activeTab?.id,
        });
      } else {
        const selectedTabs = validBatchTabs.filter((tabCandidate) => tabCandidate.selected);
        await sendToApp(
          selectedTabs.map((tabCandidate) => tabCandidate.url),
          {
            withCookies: sendCookies.checked,
            mode: "batch",
            items: selectedTabs.map((tabCandidate) => ({
              url: tabCandidate.url,
              tabId: tabCandidate.id,
            })),
          },
        );
      }

      btnText.textContent = translate("popupSent");
      if (currentMode === "standard") {
        setStatusPill("ok", "popupSent");
      }

      // 1.5 秒后自动恢复按钮为可点击状态，允许用户再次触发
      setTimeout(() => {
        isSubmitting = false;
        updateSendBtnState();
      }, 1500);
    } catch (error) {
      console.error("[YDL GUI] send failed:", error);
      btnText.textContent = translate("popupFailed");
      if (currentMode === "standard") {
        setStatusPill("bad", "popupFailed");
      }
      fallback.hidden = false;
      isSubmitting = false;
      sendBtn.disabled = false;
    }
  });
});
