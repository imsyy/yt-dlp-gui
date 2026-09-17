import { invoke } from "@tauri-apps/api/core";
import type { DownloadTask } from "@/types";

/**
 * 从下载任务中解析提取可能的目标文件绝对路径
 * 包括 outputFile 以及从 yt-dlp 日志中打印的 Destination 等
 */
export function extractTaskFilePaths(task: DownloadTask): string[] {
  const paths = new Set<string>();

  if (task.outputFile && task.outputFile.trim()) {
    paths.add(task.outputFile.trim());
  }

  if (task.logs && Array.isArray(task.logs)) {
    for (const log of task.logs) {
      if (typeof log !== "string") continue;
      const trimmed = log.trim();

      // [download] Destination: D:\path\to\file.ext
      const destMatch = trimmed.match(/^\[download\]\s+Destination:\s+(.+)$/);
      if (destMatch && destMatch[1]) {
        paths.add(destMatch[1].trim());
        continue;
      }

      // [download] D:\path\to\file.ext has already been downloaded
      const alreadyMatch = trimmed.match(/^\[download\]\s+(.+)\s+has already been downloaded$/);
      if (alreadyMatch && alreadyMatch[1]) {
        paths.add(alreadyMatch[1].trim());
        continue;
      }

      // [Merger] Merging formats into "D:\path\to\file.ext"
      const mergeMatch = trimmed.match(/\[Merger\]\s+Merging formats into\s+"([^"]+)"/);
      if (mergeMatch && mergeMatch[1]) {
        paths.add(mergeMatch[1].trim());
      }
    }
  }

  return Array.from(paths);
}

/**
 * 清理单个任务关联的所有残留文件（包括 .part、.ytdl、临时分片等）
 */
export async function cleanTaskResiduals(task: DownloadTask): Promise<string[]> {
  try {
    const knownPaths = extractTaskFilePaths(task);
    const deleted = await invoke<string[]>("clean_task_residual_files", {
      downloadDir: task.params?.downloadDir || "",
      title: task.title || "",
      knownPaths,
    });
    return deleted || [];
  } catch (error) {
    console.warn("Failed to clean task residuals:", error);
    return [];
  }
}

/**
 * 批量清理多个任务的残留文件
 */
export async function cleanMultipleTasksResiduals(tasks: DownloadTask[]): Promise<string[]> {
  const allDeleted: string[] = [];
  for (const task of tasks) {
    const deleted = await cleanTaskResiduals(task);
    allDeleted.push(...deleted);
  }
  return allDeleted;
}
