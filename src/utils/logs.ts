/**
 * 下载任务日志环形缓冲
 *
 * yt-dlp 子进程的输出是无界的（长任务/播放列表可达数千行），直接全量存内存
 * 会导致渲染卡顿与 SQLite 膨胀。这里采用"头部 + 尾部"截断策略：
 * 保留前 50 行（含 [download] Destination 等定位残留文件所需的路径行），
 * 其余只保留最新尾部，中间丢弃并插入一行省略号占位。
 *
 * 注意：截断占位行就是 plain "..."，与语言无关，不需要翻译。
 */

/** 单个任务内存中保留的最大日志行数 */
export const MAX_TASK_LOGS = 500;
/** 截断时永久保留的头部行数（包含任务早期的目标路径等关键行） */
export const HEAD_TASK_LOGS = 50;
/**
 * 日志截断占位行（中间被截掉的部分用一行省略号表示）
 */
export const LOG_TRUNCATED_MARKER = "...";

/** 判断一行是否为截断占位行 */
export const isTruncatedMarker = (line: string): boolean => line === LOG_TRUNCATED_MARKER;

/**
 * 向任务日志追加一行，超出上限时自动截断中间部分
 *
 * @param logs 任务日志数组（会被原地修改）
 * @param line 新增日志行
 */
export const pushTaskLog = (logs: string[], line: string): void => {
  if (logs[HEAD_TASK_LOGS] === LOG_TRUNCATED_MARKER) {
    // 已进入截断稳态：丢掉省略号之后最旧的一行
    logs.splice(HEAD_TASK_LOGS + 1, 1);
  } else if (logs.length >= MAX_TASK_LOGS) {
    // 首次触顶：丢掉尾部区最旧的一行，插入省略号后再腾出一个空位
    logs.splice(HEAD_TASK_LOGS, 1);
    logs.splice(HEAD_TASK_LOGS, 0, LOG_TRUNCATED_MARKER);
    logs.splice(HEAD_TASK_LOGS + 1, 1);
  }
  logs.push(line);
};

/**
 * 将已存在的日志数组裁剪到上限以内（用于加载历史数据/迁移老数据时）
 *
 * @param logs 任务日志数组（会被原地修改）
 */
export const trimTaskLogs = (logs: string[]): void => {
  const clean = logs.filter((line) => !isTruncatedMarker(line));
  if (clean.length <= MAX_TASK_LOGS) {
    if (clean.length !== logs.length) {
      logs.length = 0;
      logs.push(...clean);
    }
    return;
  }
  const head = clean.slice(0, HEAD_TASK_LOGS);
  const tail = clean.slice(-(MAX_TASK_LOGS - HEAD_TASK_LOGS - 1));
  logs.length = 0;
  logs.push(...head, LOG_TRUNCATED_MARKER, ...tail);
};
