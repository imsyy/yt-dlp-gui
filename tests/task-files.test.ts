import { describe, expect, it } from "vitest";
import { extractTaskFilePaths } from "@/utils/taskFiles";
import type { DownloadTask } from "@/types";

describe("extractTaskFilePaths", () => {
  it("extracts paths from outputFile and diverse yt-dlp logs", () => {
    const mockTask = {
      id: "task_1",
      title: "Test Video",
      outputFile: "D:/Downloads/final_video.mp4",
      logs: [
        "[download] Destination: D:/Downloads/Test Video.f137.mp4",
        "[download] 10% of 100MiB at 10MiB/s",
        "[download] Destination: D:/Downloads/Test Video.f140.m4a",
        "[Merger] Merging formats into \"D:/Downloads/Test Video.mp4\"",
        "[download] D:/Downloads/already_downloaded.mp4 has already been downloaded",
      ],
      params: { downloadDir: "D:/Downloads" },
    } as unknown as DownloadTask;

    const paths = extractTaskFilePaths(mockTask);
    expect(paths).toContain("D:/Downloads/final_video.mp4");
    expect(paths).toContain("D:/Downloads/Test Video.f137.mp4");
    expect(paths).toContain("D:/Downloads/Test Video.f140.m4a");
    expect(paths).toContain("D:/Downloads/Test Video.mp4");
    expect(paths).toContain("D:/Downloads/already_downloaded.mp4");
    expect(paths).toHaveLength(5);
  });

  it("handles empty logs or undefined outputFile gracefully", () => {
    const mockTask = {
      id: "task_2",
      title: "Empty Logs Task",
      logs: [],
      params: { downloadDir: "D:/Downloads" },
    } as unknown as DownloadTask;

    const paths = extractTaskFilePaths(mockTask);
    expect(paths).toEqual([]);
  });
});
