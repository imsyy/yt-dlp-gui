const DEFAULT_LANGUAGE = "en";
const SUPPORTED_LANGUAGES = new Set(["en", "zh_CN"]);
let cachedLanguage;
let cachedMessages;

export async function getLanguage() {
  if (cachedLanguage) return cachedLanguage;
  const stored = await chrome.storage.local.get("language");
  cachedLanguage = SUPPORTED_LANGUAGES.has(stored.language)
    ? stored.language
    : DEFAULT_LANGUAGE;
  return cachedLanguage;
}

export async function setLanguage(language) {
  cachedLanguage = SUPPORTED_LANGUAGES.has(language) ? language : DEFAULT_LANGUAGE;
  cachedMessages = undefined;
  await chrome.storage.local.set({ language: cachedLanguage });
}

export function resetI18n() {
  cachedLanguage = undefined;
  cachedMessages = undefined;
}

export async function createTranslator() {
  const language = await getLanguage();
  if (!cachedMessages) {
    const response = await fetch(chrome.runtime.getURL(`_locales/${language}/messages.json`));
    cachedMessages = await response.json();
  }
  return (key) => cachedMessages[key]?.message || key;
}
