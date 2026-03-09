import { createI18n } from "vue-i18n";
import zhCN from "./zh-CN.json";
import enUS from "./en-US.json";
import jaJP from "./ja-JP.json";
import koKR from "./ko-KR.json";
import esES from "./es-ES.json";
import ruRU from "./ru-RU.json";
import zhTW from "./zh-TW.json";

// ==================== 语言注册表（新增语言只改这里 + 创建翻译文件） ====================

export interface LocaleEntry {
  /** 语言代码 */
  code: string;
  /** 国旗 emoji */
  flag: string;
  /** 原生显示名称 */
  label: string;
  /** navigator.language 前缀匹配规则 */
  match: (lang: string) => boolean;
}

export const localeEntries: LocaleEntry[] = [
  { code: "en-US", flag: "🇺🇸", label: "English", match: (lang) => lang.startsWith("en") },
  { code: "zh-CN", flag: "🇨🇳", label: "简体中文", match: (lang) => lang === "zh-CN" || lang === "zh-SG" || lang === "zh" },
  { code: "zh-TW", flag: "🇭🇰", label: "繁體中文", match: (lang) => lang.startsWith("zh") },
  { code: "ja-JP", flag: "🇯🇵", label: "日本語", match: (lang) => lang.startsWith("ja") },
  { code: "ko-KR", flag: "🇰🇷", label: "한국어", match: (lang) => lang.startsWith("ko") },
  { code: "es-ES", flag: "🇪🇸", label: "Español", match: (lang) => lang.startsWith("es") },
  { code: "ru-RU", flag: "🇷🇺", label: "Русский", match: (lang) => lang.startsWith("ru") },
];

/** locale code → entry 快速查找 */
const localeMap = new Map(localeEntries.map((e) => [e.code, e]));

// ==================== 工具函数 ====================

/** 根据系统语言返回最匹配的 locale code */
const getSystemLocale = (): string => {
  const lang = navigator.language;
  const matched = localeEntries.find((e) => e.match(lang));
  return matched ? matched.code : localeEntries[0].code;
};

/** 从 localStorage 读取用户的语言偏好 */
const getSavedLocale = (): string | null => {
  try {
    const setting = localStorage.getItem("setting");
    if (setting) {
      const parsed = JSON.parse(setting);
      return parsed.locale || null;
    }
  } catch {
    // ignore
  }
  return null;
};

/** 将 locale 值解析为实际 code */
export const resolveLocale = (locale: string): string => {
  if (!locale) return getSystemLocale();
  return localeMap.has(locale) ? locale : getSystemLocale();
};

// ==================== i18n 实例 ====================

const savedLocale = getSavedLocale();
const defaultLocale = resolveLocale(savedLocale ?? "auto");

const i18n = createI18n({
  legacy: false,
  locale: defaultLocale,
  fallbackLocale: "en-US",
  messages: {
    "zh-CN": zhCN,
    "en-US": enUS,
    "ja-JP": jaJP,
    "ko-KR": koKR,
    "es-ES": esES,
    "ru-RU": ruRU,
    "zh-TW": zhTW,
  },
});

/** 切换语言（供 settings store 调用） */
export const setI18nLocale = (locale: string) => {
  const resolved = resolveLocale(locale);
  (i18n.global.locale as unknown as { value: string }).value = resolved;
  document.documentElement.lang = resolved;
};

// 初始化时同步 html lang
document.documentElement.lang = defaultLocale;

export default i18n;
