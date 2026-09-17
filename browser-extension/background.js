/**
 * Background service worker for YDL GUI Helper.
 * 负责：
 *   - 扩展安装与启动时注册右键上下文菜单
 *   - 监听活动标签页变化并同步更新工具栏角标（Badge）状态
 *   - 通过本地 HTTP 桥接将当前网页、选中文本链接或全部标签页发送至桌面客户端
 */

import { sendToApp as sendRequest } from "./bridge.js";
import { createTranslator, resetI18n } from "./i18n.js";

/**
 * 校验指定 URL 是否为受支持的 HTTP/HTTPS 协议网络链接
 *
 * @param {string} url 待检测的地址字符串
 * @returns {boolean} 是否为有效的网络地址
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
 * 触发系统级 Chrome 通知提醒用户导入状态
 *
 * @param {string} messageKey 语言包消息键名
 * @returns {Promise<void>}
 */
const notify = async (messageKey) => {
  const translator = await createTranslator();
  chrome.notifications?.create({
    type: "basic",
    iconUrl: "icons/icon128.png",
    title: translator("notifyTitle"),
    message: translator(messageKey),
  });
};

/**
 * 筛选并发送网页链接及 Cookie 至 YDL GUI 桌面客户端
 *
 * @param {string | string[]} target 单个目标链接或链接数组
 * @param {object} [options={}] 配置项
 * @param {boolean} [options.withCookies=true] 是否附带 Cookie
 * @param {"standard" | "batch"} [options.mode="standard"] 导入模式（标准或批量）
 * @param {number} [options.tabId] 来源标签页 ID
 * @param {Array<{url: string, tabId?: number}>} [options.items=[]] 批量标签项信息
 * @returns {Promise<void>}
 */
const sendToApp = async (
  target,
  { withCookies = true, mode = "standard", tabId, items = [] } = {},
) => {
  let targetUrls = [];
  if (Array.isArray(target)) {
    targetUrls = target.filter(isSupportedUrl);
  } else if (typeof target === "string" && isSupportedUrl(target)) {
    targetUrls = [target];
  }

  if (targetUrls.length === 0) {
    notify("notifyUnsupported");
    return;
  }

  try {
    const isSuccess = await sendRequest(targetUrls, {
      withCookies,
      mode,
      tabId,
      items,
    });
    notify(isSuccess ? "notifySent" : "notifyFailed");
  } catch {
    notify("notifyFailed");
  }
};

// Context menus

/**
 * 注册浏览器右键上下文菜单（支持页面、链接、划词文本与扩展图标右键全标签页导入）
 *
 * @returns {Promise<void>}
 */
const setupMenus = async () => {
  const translator = await createTranslator();
  chrome.contextMenus.removeAll(() => {
    chrome.contextMenus.create({
      id: "ydl-send-page",
      title: translator("menuSendPage"),
      contexts: ["page", "frame"],
      documentUrlPatterns: ["http://*/*", "https://*/*"],
    });
    chrome.contextMenus.create({
      id: "ydl-send-link",
      title: translator("menuSendLink"),
      contexts: ["link"],
    });
    chrome.contextMenus.create({
      id: "ydl-send-selection",
      title: translator("menuSendSelection"),
      contexts: ["selection"],
    });
    chrome.contextMenus.create({
      id: "ydl-send-all-tabs",
      title: translator("menuSendAllTabs"),
      contexts: ["action"],
    });
  });
};

chrome.runtime.onInstalled.addListener(setupMenus);
chrome.runtime.onStartup.addListener(setupMenus);
chrome.storage.onChanged.addListener((changes) => {
  if (changes.language) {
    resetI18n();
    setupMenus();
  }
});

chrome.contextMenus.onClicked.addListener((menuInfo, activeTab) => {
  if (menuInfo.menuItemId === "ydl-send-page") {
    const targetUrl = menuInfo.frameUrl || menuInfo.pageUrl || activeTab?.url;
    if (targetUrl) {
      sendToApp(targetUrl, { mode: "standard", tabId: activeTab?.id });
    }
    return;
  }

  if (menuInfo.menuItemId === "ydl-send-link") {
    if (menuInfo.linkUrl) {
      sendToApp(menuInfo.linkUrl, { mode: "standard", tabId: activeTab?.id });
    }
    return;
  }

  if (menuInfo.menuItemId === "ydl-send-selection") {
    const selectionContent = (menuInfo.selectionText || "").trim();
    // 选中文本可能夹带空白字符，提取首个符合 http(s) 的链接
    const matchedUrl = selectionContent
      .split(/\s+/)
      .find((token) => /^https?:\/\//i.test(token));
    if (matchedUrl) {
      sendToApp(matchedUrl, { mode: "standard", tabId: activeTab?.id });
    } else {
      notify("notifyUnsupported");
    }
    return;
  }

  if (menuInfo.menuItemId === "ydl-send-all-tabs") {
    chrome.tabs.query({ currentWindow: true }).then((allTabs) => {
      const validTabs = (allTabs || []).filter((tabItem) => isSupportedUrl(tabItem.url));
      if (validTabs.length > 0) {
        sendToApp(
          validTabs.map((tabItem) => tabItem.url),
          {
            mode: "batch",
            items: validTabs.map((tabItem) => ({ url: tabItem.url, tabId: tabItem.id })),
          },
        );
      } else {
        notify("notifyUnsupported");
      }
    });
  }
});

/**
 * 清理指定或全局扩展角标，彻底移除右下角遮挡大方块，恢复原始清爽干净的图标外观
 *
 * @param {number} [targetTabId] 标签页 ID（可选）
 * @returns {Promise<void>}
 */
const clearActionBadge = async (targetTabId) => {
  try {
    if (typeof targetTabId === "number") {
      await chrome.action.setBadgeText({ text: "", tabId: targetTabId });
    } else {
      await chrome.action.setBadgeText({ text: "" });
    }
  } catch {
    // 忽略标签页可能已关闭的异常
  }
};

chrome.tabs.onActivated.addListener(async ({ tabId: activeTabId }) => {
  await clearActionBadge(activeTabId);
});

chrome.tabs.onUpdated.addListener((updatedTabId) => {
  void clearActionBadge(updatedTabId);
});

// 扩展安装或启动时重置全局角标
chrome.runtime.onInstalled.addListener(() => void clearActionBadge());
chrome.runtime.onStartup.addListener(() => void clearActionBadge());
