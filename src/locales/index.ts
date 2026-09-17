import { createI18n } from "vue-i18n";
import type { LocaleMessageValue } from "vue-i18n";

export interface LocaleEntry {
  /** 语言代码 */
  code: string;
  /** 国旗 emoji */
  flag: string;
  /** 原生显示名称 */
  label: string;
  /** navigator.language 前缀匹配规则 */
  match: (languageCandidate: string) => boolean;
  /** 是否为从右向左书写的语言 */
  rtl?: boolean;
}

// 顺序按 ISO 639-1 语言代码字母序
export const localeEntries: LocaleEntry[] = [
  {
    code: "ar-EG",
    flag: "🇪🇬",
    label: "العربية",
    match: (languageCandidate) => languageCandidate.startsWith("ar"),
    rtl: true,
  },
  {
    code: "de-DE",
    flag: "🇩🇪",
    label: "Deutsch",
    match: (languageCandidate) => languageCandidate.startsWith("de"),
  },
  {
    code: "en-US",
    flag: "🇺🇸",
    label: "English",
    match: (languageCandidate) => languageCandidate.startsWith("en"),
  },
  {
    code: "es-ES",
    flag: "🇪🇸",
    label: "Español",
    match: (languageCandidate) => languageCandidate.startsWith("es"),
  },
  {
    code: "fr-FR",
    flag: "🇫🇷",
    label: "Français",
    match: (languageCandidate) => languageCandidate.startsWith("fr"),
  },
  {
    code: "hu-HU",
    flag: "🇭🇺",
    label: "Magyar",
    match: (languageCandidate) => languageCandidate.startsWith("hu"),
  },
  {
    code: "ja-JP",
    flag: "🇯🇵",
    label: "日本語",
    match: (languageCandidate) => languageCandidate.startsWith("ja"),
  },
  {
    code: "ko-KR",
    flag: "🇰🇷",
    label: "한국어",
    match: (languageCandidate) => languageCandidate.startsWith("ko"),
  },
  {
    code: "pt-BR",
    flag: "🇧🇷",
    label: "Português",
    match: (languageCandidate) => languageCandidate.startsWith("pt"),
  },
  {
    code: "ru-RU",
    flag: "🇷🇺",
    label: "Русский",
    match: (languageCandidate) => languageCandidate.startsWith("ru"),
  },
  {
    code: "uk-UA",
    flag: "🇺🇦",
    label: "Українська",
    match: (languageCandidate) => languageCandidate.startsWith("uk"),
  },
  {
    code: "vi-VN",
    flag: "🇻🇳",
    label: "Tiếng Việt",
    match: (languageCandidate) => languageCandidate.startsWith("vi"),
  },
  {
    code: "sk-SK",
    flag: "🇸🇰",
    label: "Slovenčina",
    match: (languageCandidate) => languageCandidate.startsWith("sk"),
  },
  {
    code: "zh-CN",
    flag: "🇨🇳",
    label: "简体中文",
    match: (languageCandidate) =>
      languageCandidate === "zh-CN" || languageCandidate === "zh-SG" || languageCandidate === "zh",
  },
  {
    code: "zh-TW",
    flag: "🇭🇰",
    label: "繁體中文",
    match: (languageCandidate) => languageCandidate.startsWith("zh"),
  },
];

/** locale code → entry 快速查找映射表 */
const localeMap = new Map(localeEntries.map((localeEntry) => [localeEntry.code, localeEntry]));

/**
 * 自动扫描并以同步方式加载各语言子目录下的所有模块 JSON 文件
 * 文件路径格式形如：./zh-CN/home.json, ./en-US/settings.json
 */
const localeModules = import.meta.glob<{ default: Record<string, LocaleMessageValue> }>(
  "./*/*.json",
  {
    eager: true,
  },
);

/**
 * 遍历扫描结果，将各语言子目录下的模块 JSON 深度合并为完整的命名空间字典树
 *
 * @returns 包含所有语言翻译数据的聚合对象
 */
const loadLocaleMessages = (): Record<string, Record<string, LocaleMessageValue>> => {
  const aggregatedMessages: Record<string, Record<string, LocaleMessageValue>> = {};

  for (const moduleFilePath in localeModules) {
    const matchedPath = moduleFilePath.match(/\.\/([^/]+)\/([^/]+)\.json$/);
    if (!matchedPath) continue;

    const [, localeCode] = matchedPath;
    if (!aggregatedMessages[localeCode]) {
      aggregatedMessages[localeCode] = {};
    }

    const exportedContent = localeModules[moduleFilePath].default;
    Object.assign(aggregatedMessages[localeCode], exportedContent);
  }

  return aggregatedMessages;
};

/**
 * 根据系统语言环境返回最匹配的 locale 代码；未匹配时兜底降级至英文
 *
 * @returns 匹配到的语言代码
 */
const getSystemLocale = (): string => {
  const systemLanguage = navigator.language;
  const matchedEntry = localeEntries.find((localeEntry) => localeEntry.match(systemLanguage));
  return matchedEntry ? matchedEntry.code : "en-US";
};

/**
 * 从本地持久化存储中读取用户预设的语言偏好
 *
 * @returns 用户选择的语言代码或 null
 */
const getSavedLocale = (): string | null => {
  try {
    const persistedSetting = localStorage.getItem("setting");
    if (persistedSetting) {
      const parsedSetting = JSON.parse(persistedSetting);
      return parsedSetting.locale || null;
    }
  } catch {
    // 忽略解析异常
  }
  return null;
};

/**
 * 将传入的 locale 参数值校验并解析为系统支持的实际语言代码
 *
 * @param targetLocale 目标语言代码字符串
 * @returns 校验合法的实际语言代码
 */
export const resolveLocale = (targetLocale: string): string => {
  if (!targetLocale) return getSystemLocale();
  return localeMap.has(targetLocale) ? targetLocale : getSystemLocale();
};

/**
 * 根据指定语言代码返回当前文档书写方向（LTR 或 RTL）
 *
 * @param localeCode 目标语言代码
 * @returns "rtl" 或 "ltr"
 */
const getDirection = (localeCode: string): "rtl" | "ltr" =>
  localeMap.get(localeCode)?.rtl ? "rtl" : "ltr";

const savedLocalePreference = getSavedLocale();
const defaultLocaleCode = resolveLocale(savedLocalePreference ?? "auto");

const i18n = createI18n({
  legacy: false,
  locale: defaultLocaleCode,
  fallbackLocale: "en-US",
  messages: loadLocaleMessages(),
});

/**
 * 动态切换当前应用运行时的多语言环境与文档书写方向
 *
 * @param nextLocale 待切换的目标语言代码
 */
export const setI18nLocale = (nextLocale: string): void => {
  const resolvedCode = resolveLocale(nextLocale);
  i18n.global.locale.value = resolvedCode;
  document.documentElement.lang = resolvedCode;
  document.documentElement.dir = getDirection(resolvedCode);
};

// 初始化时同步 html lang 和 dir 属性
document.documentElement.lang = defaultLocaleCode;
document.documentElement.dir = getDirection(defaultLocaleCode);

export default i18n;
