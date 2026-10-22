export const DEFAULT_OUTPUT_TEMPLATE = "%(title).200s [%(id)s].%(ext)s";

/** 扩展名后缀：最终传给 yt-dlp 的模板必须恰好以它结尾。 */
export const EXT_SUFFIX = ".%(ext)s";

/**
 * 官方 OUTPUT TEMPLATE 文档允许的转换类型：
 * printf 标准 diouxXeEfFgGcrs + yt-dlp 扩展 B（字节） j（json） h（HTML转义）
 * l（列表） q（终端引用） D（十进制后缀） S（文件名净化） U（Unicode规范化）。
 * 百分号字面量 %% 单独处理。
 */
const ALLOWED_CONVERSIONS = new Set("diouxXeEfFgGcrsBjhlqDSU".split(""));

export type TemplateErrorCode = "empty" | "lone-percent" | "unclosed" | "bad-conversion";

export interface TemplateError {
  code: TemplateErrorCode;
  /** 触发错误的字符（转换类型或孤立 % 后的字符），用于错误文案插值 */
  char: string;
}

export const TEMPLATE_ERROR_KEYS: Record<TemplateErrorCode, string> = {
  empty: "detail.tplErrEmpty",
  "lone-percent": "detail.tplErrPercent",
  unclosed: "detail.tplErrUnclosed",
  "bad-conversion": "detail.tplErrConversion",
};

/**
 * 按官方 OUTPUT TEMPLATE 语法校验模板。
 * 只判明确非法的三种情况（孤立 %、未闭合 %(、未知转换类型），其余一律放行，
 * 避免对冷门但合法的写法（如对象遍历、切片、条件替换）误报。
 * 注意：缺 .%(ext)s 不算错，由 normalizeOutputTemplate 自动补齐。
 */
export const validateOutputTemplate = (template: string): TemplateError | null => {
  const text = template.trim();
  if (!text) return { code: "empty", char: "" };
  let i = 0;
  while (i < text.length) {
    if (text[i] !== "%") {
      i += 1;
      continue;
    }
    const next = text[i + 1];
    // %% 为百分号字面量（官方文档：To use percent literals in an output template use %%）
    if (next === "%") {
      i += 2;
      continue;
    }
    if (next !== "(") return { code: "lone-percent", char: next ?? "" };
    const close = text.indexOf(")", i + 2);
    if (close === -1) return { code: "unclosed", char: "" };
    // 跳过 flags / width / precision（如 05、.200、+、#），取其后一位作为转换类型
    let j = close + 1;
    while (j < text.length && /[-+ #0-9.*]/.test(text[j])) j += 1;
    const conversion = text[j];
    if (!conversion || !ALLOWED_CONVERSIONS.has(conversion)) {
      return { code: "bad-conversion", char: conversion ?? "" };
    }
    i = j + 1;
  }
  return null;
};

/** 规范化模板：去首尾空格、空模板回退默认，并保证恰好以一个 .%(ext)s 结尾。 */
export const normalizeOutputTemplate = (template: string): string => {
  const text = template.trim() || DEFAULT_OUTPUT_TEMPLATE;
  const base = text.endsWith(EXT_SUFFIX) ? text.slice(0, -EXT_SUFFIX.length) : text;
  return `${base}${EXT_SUFFIX}`;
};

/** 将持久化的静态前后缀组合到 yt-dlp 模板中，后缀始终位于扩展名前。 */
export const composeOutputTemplate = (
  template: string,
  prefix: string,
  suffix: string,
  playlistSubfolder = false,
): string => {
  const normalized = normalizeOutputTemplate(template);
  const base = normalized.slice(0, -EXT_SUFFIX.length);
  let separator = -1;
  for (let index = 0; index < base.length; index += 1) {
    if (base[index] === "%" && base[index + 1] === "(") {
      const close = base.indexOf(")", index + 2);
      if (close !== -1) index = close;
    } else if (base[index] === "/" || base[index] === "\\") {
      separator = index;
    }
  }
  const directory = base.slice(0, separator + 1);
  const filename = base.slice(separator + 1);
  const playlistDirectory = playlistSubfolder ? "%(playlist_title,playlist_id|Playlist)s/" : "";
  return `${directory}${playlistDirectory}${prefix}${filename}${suffix}${EXT_SUFFIX}`;
};
