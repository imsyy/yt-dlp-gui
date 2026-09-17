/**
 * 老版本数据迁移模块的类型定义
 */

/** 单个迁移域的状态：pending 待处理 / done 已完成 */
export type MigrationDomainStatus = "pending" | "done";

/**
 * 老数据迁移总账（localStorage）
 *
 * 整个 src/migration 文件夹可在迁移稳定后整体删除。
 */
export interface MigrationLedger {
  tasks: MigrationDomainStatus;
  history: MigrationDomainStatus;
}

/** 检测到的老版本数据量（仅计数，不读全量） */
export interface LegacyDataSummary {
  tasks: number;
  history: number;
}

/** 是否需要弹窗询问用户 */
export const needsMigrationPrompt = (summary: LegacyDataSummary): boolean =>
  summary.tasks > 0 || summary.history > 0;

/** 迁移执行进度（弹窗进度态展示用） */
export interface MigrationProgress {
  domain: "tasks" | "history";
  done: number;
  total: number;
}
