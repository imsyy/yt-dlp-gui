/**
 * 国际化（i18n）辅助模块
 * 负责管理扩展语言偏好设置、加载对应的 messages.json 语言包并提供翻译函数。
 */

const DEFAULT_LANGUAGE = "en";
const SUPPORTED_LANGUAGES = new Set(["en", "zh_CN"]);
let cachedLanguage;
let cachedMessages;

/**
 * 获取当前已配置或默认使用的语言标识
 *
 * @returns {Promise<string>} 语言代码（如 "zh_CN" 或 "en"）
 */
export const getLanguage = async () => {
  if (cachedLanguage) return cachedLanguage;
  const storedConfig = await chrome.storage.local.get("language");
  cachedLanguage = SUPPORTED_LANGUAGES.has(storedConfig.language)
    ? storedConfig.language
    : DEFAULT_LANGUAGE;
  return cachedLanguage;
};

/**
 * 切换并持久化扩展语言设置
 *
 * @param {string} nextLanguage 目标语言标识
 * @returns {Promise<void>}
 */
export const setLanguage = async (nextLanguage) => {
  cachedLanguage = SUPPORTED_LANGUAGES.has(nextLanguage)
    ? nextLanguage
    : DEFAULT_LANGUAGE;
  cachedMessages = undefined;
  await chrome.storage.local.set({ language: cachedLanguage });
};

/**
 * 清理语言与词条缓存（通常在语言变更时调用）
 */
export const resetI18n = () => {
  cachedLanguage = undefined;
  cachedMessages = undefined;
};

/**
 * 异步初始化并创建多语言翻译函数
 *
 * @returns {Promise<(key: string) => string>} 接收多语言键名并返回翻译文本的高阶函数
 */
export const createTranslator = async () => {
  const language = await getLanguage();
  if (!cachedMessages) {
    const localeUrl = chrome.runtime.getURL(`_locales/${language}/messages.json`);
    const response = await fetch(localeUrl);
    cachedMessages = await response.json();
  }
  return (translationKey) => cachedMessages[translationKey]?.message || translationKey;
};
