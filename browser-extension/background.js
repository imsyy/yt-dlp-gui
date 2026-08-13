/**
 * Background service worker for YDL GUI Helper.
 * Responsibilities:
 *   - register right-click context menus on install/startup
 *   - keep the action badge in sync with whether the active tab is supported
 *   - deliver requests through the acknowledged local bridge
 */

import { sendToApp as sendRequest } from "./bridge.js";
import { createTranslator, resetI18n } from "./i18n.js";

const BADGE_COLOR = "#18A058";

function isSupportedUrl(url) {
  if (!url) return false;
  try {
    return ["http:", "https:"].includes(new URL(url).protocol);
  } catch {
    return false;
  }
}

async function notify(messageKey) {
  const t = await createTranslator();
  chrome.notifications?.create({
    type: "basic",
    iconUrl: "icons/icon128.png",
    title: t("notifyTitle"),
    message: t(messageKey),
  });
}

async function sendToApp(videoUrl, { withCookies = true, tabId } = {}) {
  if (!isSupportedUrl(videoUrl)) {
    notify("notifyUnsupported");
    return;
  }
  try {
    const result = await sendRequest(videoUrl, withCookies, tabId);
    notify(result ? "notifySent" : "notifyFailed");
  } catch {
    notify("notifyFailed");
  }
}

// ---------- Context menus ----------

async function setupMenus() {
  const t = await createTranslator();
  chrome.contextMenus.removeAll(() => {
    chrome.contextMenus.create({
      id: "ydl-send-page",
      title: t("menuSendPage"),
      contexts: ["page", "frame"],
      documentUrlPatterns: ["http://*/*", "https://*/*"],
    });
    chrome.contextMenus.create({
      id: "ydl-send-link",
      title: t("menuSendLink"),
      contexts: ["link"],
    });
    chrome.contextMenus.create({
      id: "ydl-send-selection",
      title: t("menuSendSelection"),
      contexts: ["selection"],
    });
  });
}

chrome.runtime.onInstalled.addListener(setupMenus);
chrome.runtime.onStartup.addListener(setupMenus);
chrome.storage.onChanged.addListener((changes) => {
  if (changes.language) {
    resetI18n();
    setupMenus();
  }
});

chrome.contextMenus.onClicked.addListener((info, tab) => {
  if (info.menuItemId === "ydl-send-page") {
    const target = info.frameUrl || info.pageUrl || tab?.url;
    if (target) sendToApp(target, { tabId: tab?.id });
    return;
  }
  if (info.menuItemId === "ydl-send-link") {
    if (info.linkUrl) sendToApp(info.linkUrl, { tabId: tab?.id });
    return;
  }
  if (info.menuItemId === "ydl-send-selection") {
    const text = (info.selectionText || "").trim();
    // selection might wrap http(s) URL with whitespace — use first token.
    const candidate = text.split(/\s+/).find((s) => /^https?:\/\//i.test(s));
    if (candidate) sendToApp(candidate, { tabId: tab?.id });
    else notify("notifyUnsupported");
  }
});

// ---------- Action badge ----------

async function updateBadge(tabId, url) {
  const supported = isSupportedUrl(url);
  try {
    const t = await createTranslator();
    await chrome.action.setBadgeBackgroundColor({ color: BADGE_COLOR, tabId });
    await chrome.action.setBadgeText({ text: supported ? t("badgeOn") : "", tabId });
  } catch {
    // tab might be gone; ignore
  }
}

chrome.tabs.onActivated.addListener(async ({ tabId }) => {
  try {
    const tab = await chrome.tabs.get(tabId);
    updateBadge(tabId, tab.url);
  } catch {}
});

chrome.tabs.onUpdated.addListener((tabId, changeInfo, tab) => {
  if (changeInfo.url || changeInfo.status === "complete") {
    updateBadge(tabId, tab.url);
  }
});
