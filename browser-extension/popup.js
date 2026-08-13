/**
 * Popup script for YDL GUI Helper.
 * Handles: i18n hydration, supported-site detection, deep link dispatch,
 * and a fallback hint when the desktop app likely isn't installed.
 */

import { sendToApp } from "./bridge.js";
import { createTranslator, getLanguage, setLanguage } from "./i18n.js";

let t = (key) => key;

function isSupportedUrl(url) {
  if (!url) return false;
  try {
    return ["http:", "https:"].includes(new URL(url).protocol);
  } catch {
    return false;
  }
}

/** Hydrate every [data-i18n] node with its localized message. */
function applyI18n() {
  document.querySelectorAll("[data-i18n]").forEach((el) => {
    const key = el.getAttribute("data-i18n");
    const msg = t(key);
    if (msg) el.textContent = msg;
  });
}

function setStatus(kind, key) {
  const el = document.getElementById("status");
  const text = document.getElementById("status-text");
  el.classList.remove("pill-ok", "pill-bad", "pill-muted");
  el.classList.add(
    kind === "ok" ? "pill-ok" : kind === "bad" ? "pill-bad" : "pill-muted"
  );
  text.textContent = t(key);
}

document.addEventListener("DOMContentLoaded", async () => {
  t = await createTranslator();
  applyI18n();

  const language = document.getElementById("language");
  language.value = await getLanguage();
  language.addEventListener("change", async () => {
    await setLanguage(language.value);
    window.location.reload();
  });

  const sendBtn = document.getElementById("send-btn");
  const btnText = document.getElementById("btn-text");
  const urlPreview = document.getElementById("url-preview");
  const sendCookies = document.getElementById("send-cookies");
  const cookieRow = document.getElementById("cookie-row");
  const fallback = document.getElementById("fallback");

  const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
  const pageUrl = tab?.url || "";

  if (!isSupportedUrl(pageUrl)) {
    setStatus("bad", "popupUnsupported");
    sendBtn.disabled = true;
    sendCookies.disabled = true;
    cookieRow.style.opacity = "0.55";
    return;
  }

  setStatus("ok", "popupSupported");
  urlPreview.hidden = false;
  urlPreview.textContent = pageUrl;
  sendBtn.disabled = false;

  sendBtn.addEventListener("click", async () => {
    sendBtn.disabled = true;
    btnText.textContent = t("popupSending");
    fallback.hidden = true;

    try {
      await sendToApp(pageUrl, sendCookies.checked, tab?.id);
      btnText.textContent = t("popupSent");
      setStatus("ok", "popupSent");
    } catch (err) {
      console.error("[YDL GUI] send failed:", err);
      btnText.textContent = t("popupFailed");
      setStatus("bad", "popupFailed");
      fallback.hidden = false;
      sendBtn.disabled = false;
    }
  });
});
