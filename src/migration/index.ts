/**
 * 老版本数据（IndexedDB 下载任务 / localStorage 解析历史）迁移模块
 *
 * 结构：
 * - types.ts：总账、计数、进度等类型
 * - legacyTasks.ts：IndexedDB 任务源的读取与清洗
 * - legacyHistory.ts：localStorage 历史源的读取与清洗
 * - runner.ts：检测 → 迁移 → 校验 → 标记编排
 *
 * 约束：
 * - 本文件夹只依赖 stores 的类型，不依赖 store 实例；
 * - 落库通过 Tauri invoke，store 重载由调用方（弹窗）负责；
 * - 迁移稳定后整个文件夹可直接删除（届时 store 里已无引用）。
 */
export * from "./types";
export {
  detectLegacyData,
  migrateLegacyData,
  readLedger,
  writeLedger,
  MIGRATION_LEDGER_KEY,
  type DetectionResult,
} from "./runner";
export {
  countLegacyTasks,
  readLegacyTasks,
  sanitizeLegacyTasks,
  clearLegacyTasks,
} from "./legacyTasks";
export {
  countLegacyHistory,
  readLegacyHistory,
  parseLegacyHistory,
  clearLegacyHistory,
} from "./legacyHistory";
