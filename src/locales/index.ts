import { createI18n } from "vue-i18n";
import zhCN from "./zh-CN.json";
import enUS from "./en-US.json";
import jaJP from "./ja-JP.json";
import koKR from "./ko-KR.json";
import esES from "./es-ES.json";
import ruRU from "./ru-RU.json";
import zhTW from "./zh-TW.json";
import arEG from "./ar-EG.json";
import deDE from "./de-DE.json";
import frFR from "./fr-FR.json";
import ptBR from "./pt-BR.json";
import viVN from "./vi-VN.json";
import ukUA from "./uk-UA.json";
import skSK from "./sk-SK.json";

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
  /** 是否为从右向左书写的语言 */
  rtl?: boolean;
}

// 顺序按 ISO 639-1 语言代码字母序（世界通用顺序）；中文按地区代码细分
export const localeEntries: LocaleEntry[] = [
  {
    code: "ar-EG",
    flag: "🇪🇬",
    label: "العربية",
    match: (lang) => lang.startsWith("ar"),
    rtl: true,
  },
  { code: "de-DE", flag: "🇩🇪", label: "Deutsch", match: (lang) => lang.startsWith("de") },
  { code: "en-US", flag: "🇺🇸", label: "English", match: (lang) => lang.startsWith("en") },
  { code: "es-ES", flag: "🇪🇸", label: "Español", match: (lang) => lang.startsWith("es") },
  { code: "fr-FR", flag: "🇫🇷", label: "Français", match: (lang) => lang.startsWith("fr") },
  { code: "ja-JP", flag: "🇯🇵", label: "日本語", match: (lang) => lang.startsWith("ja") },
  { code: "ko-KR", flag: "🇰🇷", label: "한국어", match: (lang) => lang.startsWith("ko") },
  { code: "pt-BR", flag: "🇧🇷", label: "Português", match: (lang) => lang.startsWith("pt") },
  { code: "ru-RU", flag: "🇷🇺", label: "Русский", match: (lang) => lang.startsWith("ru") },
  { code: "uk-UA", flag: "🇺🇦", label: "Українська", match: (lang) => lang.startsWith("uk") },
  { code: "vi-VN", flag: "🇻🇳", label: "Tiếng Việt", match: (lang) => lang.startsWith("vi") },
  { code: "sk-SK", flag: "🇸🇰", label: "Slovenčina", match: (lang) => lang.startsWith("sk") },
  {
    code: "zh-CN",
    flag: "🇨🇳",
    label: "简体中文",
    match: (lang) => lang === "zh-CN" || lang === "zh-SG" || lang === "zh",
  },
  { code: "zh-TW", flag: "🇭🇰", label: "繁體中文", match: (lang) => lang.startsWith("zh") },
];

/** locale code → entry 快速查找 */
const localeMap = new Map(localeEntries.map((e) => [e.code, e]));

// ==================== 工具函数 ====================

/** 根据系统语言返回最匹配的 locale code；未匹配时 fallback 到英文 */
const getSystemLocale = (): string => {
  const lang = navigator.language;
  const matched = localeEntries.find((e) => e.match(lang));
  return matched ? matched.code : "en-US";
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
    "ar-EG": arEG,
    "de-DE": deDE,
    "en-US": enUS,
    "es-ES": esES,
    "fr-FR": frFR,
    "ja-JP": jaJP,
    "ko-KR": koKR,
    "pt-BR": ptBR,
    "ru-RU": ruRU,
    "uk-UA": ukUA,
    "vi-VN": viVN,
    "zh-CN": zhCN,
    "zh-TW": zhTW,
    "sk-SK": skSK,
  },
});

/** 根据 locale code 返回文档书写方向 */
const getDirection = (code: string): "rtl" | "ltr" => (localeMap.get(code)?.rtl ? "rtl" : "ltr");

/** 切换语言（供 settings store 调用） */
export const setI18nLocale = (locale: string) => {
  const resolved = resolveLocale(locale);
  (i18n.global.locale as unknown as { value: string }).value = resolved;
  document.documentElement.lang = resolved;
  document.documentElement.dir = getDirection(resolved);
};

// 初始化时同步 html lang 和 dir
document.documentElement.lang = defaultLocale;
document.documentElement.dir = getDirection(defaultLocale);

export default i18n;
