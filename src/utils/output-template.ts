export const DEFAULT_OUTPUT_TEMPLATE = "%(title).200s [%(id)s].%(ext)s";

/** 将持久化的静态前后缀组合到 yt-dlp 模板中，后缀始终位于扩展名之前。 */
export const composeOutputTemplate = (template: string, prefix: string, suffix: string): string => {
  const selectedTemplate = template.trim() || DEFAULT_OUTPUT_TEMPLATE;
  const extensionSuffix = ".%(ext)s";
  if (selectedTemplate.endsWith(extensionSuffix)) {
    return `${prefix}${selectedTemplate.slice(0, -extensionSuffix.length)}${suffix}${extensionSuffix}`;
  }
  return `${prefix}${selectedTemplate}${suffix}`;
};
