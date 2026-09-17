import { describe, expect, it } from "vitest";
import {
  HEAD_TASK_LOGS,
  LOG_TRUNCATED_MARKER,
  MAX_TASK_LOGS,
  pushTaskLog,
  trimTaskLogs,
} from "@/utils/logs";

describe("pushTaskLog", () => {
  it("appends normally below the cap", () => {
    const logs: string[] = [];
    pushTaskLog(logs, "a");
    pushTaskLog(logs, "b");
    expect(logs).toEqual(["a", "b"]);
  });

  it("keeps head + ellipsis + tail once over the cap", () => {
    const logs: string[] = [];
    const total = MAX_TASK_LOGS + 100;
    for (let i = 0; i < total; i++) {
      pushTaskLog(logs, `line-${i}`);
    }
    expect(logs.length).toBe(MAX_TASK_LOGS);
    // 头部 50 行永久保留（含早期 Destination 路径行）
    expect(logs.slice(0, HEAD_TASK_LOGS)).toEqual(
      Array.from({ length: HEAD_TASK_LOGS }, (_, i) => `line-${i}`),
    );
    expect(logs[HEAD_TASK_LOGS]).toBe(LOG_TRUNCATED_MARKER);
    expect(LOG_TRUNCATED_MARKER).toBe("...");
    // 尾部是最新的行
    expect(logs[logs.length - 1]).toBe(`line-${total - 1}`);
  });

  it("stays stable at cap on continued pushes", () => {
    const logs: string[] = [];
    for (let i = 0; i < MAX_TASK_LOGS + 500; i++) {
      pushTaskLog(logs, `line-${i}`);
    }
    expect(logs.length).toBe(MAX_TASK_LOGS);
    expect(logs.filter((l) => l === LOG_TRUNCATED_MARKER)).toHaveLength(1);
  });
});

describe("trimTaskLogs", () => {
  it("leaves short logs untouched", () => {
    const logs = ["a", "b"];
    trimTaskLogs(logs);
    expect(logs).toEqual(["a", "b"]);
  });

  it("trims oversized legacy logs to head + ellipsis + tail", () => {
    const logs = Array.from({ length: 2000 }, (_, i) => `line-${i}`);
    trimTaskLogs(logs);
    expect(logs.length).toBe(MAX_TASK_LOGS);
    expect(logs.slice(0, HEAD_TASK_LOGS)).toEqual(
      Array.from({ length: HEAD_TASK_LOGS }, (_, i) => `line-${i}`),
    );
    expect(logs[HEAD_TASK_LOGS]).toBe("...");
    expect(logs[logs.length - 1]).toBe("line-1999");
  });

  it("cleans stale markers", () => {
    const logs = ["a", "...", "b"];
    trimTaskLogs(logs);
    expect(logs).toEqual(["a", "b"]);
  });
});
