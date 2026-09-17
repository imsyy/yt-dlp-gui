import { describe, expect, it, vi, beforeEach } from "vitest";
import { normalizeTaskParams } from "@/utils/taskParams";
import { parseLegacyHistory } from "@/migration/legacyHistory";
import { sanitizeLegacyTasks } from "@/migration/legacyTasks";
import { detectLegacyData, migrateLegacyData } from "@/migration/runner";
import type { DownloadTask } from "@/types";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const { mockForageGetItem, mockForageRemoveItem } = vi.hoisted(() => ({
  mockForageGetItem: vi.fn(),
  mockForageRemoveItem: vi.fn(),
}));
vi.mock("localforage", () => ({
  default: {
    createInstance: () => ({
      getItem: mockForageGetItem,
      setItem: vi.fn(),
      removeItem: mockForageRemoveItem,
    }),
  },
}));

describe("normalizeTaskParams", () => {
  it("fills default values for partial legacy params", () => {
    const rawLegacyParams = {
      url: "https://www.youtube.com/watch?v=123",
      downloadDir: "C:/Downloads",
    };

    const normalized = normalizeTaskParams(rawLegacyParams);

    expect(normalized.url).toBe("https://www.youtube.com/watch?v=123");
    expect(normalized.downloadDir).toBe("C:/Downloads");
    expect(normalized.downloadMode).toBe("default");
    expect(normalized.videoFormat).toBeNull();
    expect(normalized.noOverwrites).toBe(false);
    expect(normalized.embedSubs).toBe(false);
    expect(normalized.subtitles).toEqual([]);
    expect(normalized.liveFromStart).toBe(false);
  });

  it("handles undefined or empty params gracefully", () => {
    const normalized = normalizeTaskParams(undefined);
    expect(normalized.url).toBe("");
    expect(normalized.downloadDir).toBe("");
    expect(normalized.downloadMode).toBe("default");
    expect(normalized.noOverwrites).toBe(false);
  });
});

const storageMap = new Map<string, string>();
const localStorageMock = {
  getItem: vi.fn((key: string) => storageMap.get(key) ?? null),
  setItem: vi.fn((key: string, val: string) => {
    storageMap.set(key, String(val));
  }),
  removeItem: vi.fn((key: string) => {
    storageMap.delete(key);
  }),
  clear: vi.fn(() => {
    storageMap.clear();
  }),
};

// mock window/localStorage in node environment
globalThis.window = {
  localStorage: localStorageMock,
} as unknown as Window & typeof globalThis;
globalThis.localStorage = localStorageMock as unknown as Storage;

describe("parseLegacyHistory", () => {
  it("parses object-style history", () => {
    const result = parseLegacyHistory(
      JSON.stringify({
        items: [
          { url: "https://example.com/v1", title: "Video 1", time: 1000 },
          { url: "  ", title: "blank", time: 0 },
        ],
      }),
    );
    expect(result).toHaveLength(1);
    expect(result[0]).toMatchObject({ url: "https://example.com/v1", title: "Video 1" });
  });

  it("parses string array-style history", () => {
    const result = parseLegacyHistory(JSON.stringify(["https://example.com/a", "  "]));
    expect(result).toHaveLength(1);
    expect(result[0].title).toBe("https://example.com/a");
  });

  it("returns empty list for corrupt input", () => {
    expect(parseLegacyHistory("not-json{{{")).toEqual([]);
    expect(parseLegacyHistory(null)).toEqual([]);
    expect(parseLegacyHistory(JSON.stringify({ items: "nope" }))).toEqual([]);
  });
});

const sampleTask = (overrides: Partial<DownloadTask> = {}): DownloadTask => ({
  id: "dl_1",
  url: "https://example.com/1",
  title: "Task 1",
  thumbnail: "",
  formatLabel: "",
  status: "completed",
  percent: 100,
  speed: "",
  eta: "",
  downloaded: "",
  total: "",
  logs: [],
  createdAt: 1000,
  params: {
    url: "https://example.com/1",
    downloadDir: "C:/DL",
  } as DownloadTask["params"],
  ...overrides,
});

describe("sanitizeLegacyTasks", () => {
  it("marks interrupted tasks as error", () => {
    const [task] = sanitizeLegacyTasks([sampleTask({ status: "downloading", id: "a" })]);
    expect(task.status).toBe("error");
    expect(task.error).toContain("中断");
    expect(task.speed).toBe("");
  });

  it("clears logs of completed tasks and trims oversized logs", () => {
    const bigLogs = Array.from({ length: 2000 }, (_, i) => `line-${i}`);
    const [completed, failed] = sanitizeLegacyTasks([
      sampleTask({ status: "completed", id: "c", logs: bigLogs }),
      sampleTask({ status: "error", id: "e", logs: bigLogs }),
    ]);
    expect(completed.logs).toEqual([]);
    expect(failed.logs.length).toBeLessThanOrEqual(500);
  });

  it("filters out tasks with empty ids", () => {
    const result = sanitizeLegacyTasks([sampleTask({ id: "  " }), sampleTask({ id: "ok" })]);
    expect(result.map((t) => t.id)).toEqual(["ok"]);
  });
});

describe("detect + migrate runner", () => {
  beforeEach(() => {
    localStorageMock.clear();
    vi.clearAllMocks();
    mockForageGetItem.mockResolvedValue(null);
  });

  it("prompts when legacy data exists, then migrates and verifies", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    const mockedInvoke = vi.mocked(invoke);

    mockForageGetItem.mockResolvedValue([sampleTask({ id: "dl_9", status: "queued" })]);
    localStorage.setItem(
      "history",
      JSON.stringify({ items: [{ url: "https://example.com/h", title: "H", time: 5 }] }),
    );

    const first = await detectLegacyData();
    expect(first.needsPrompt).toBe(true);
    expect(first.summary).toMatchObject({ tasks: 1, history: 1 });

    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "db_get_tasks") return [{ id: "dl_9" }];
      if (cmd === "db_get_history") return [{ url: "https://example.com/h" }];
      return undefined;
    });

    await migrateLegacyData(first.summary);

    expect(mockedInvoke).toHaveBeenCalledWith("db_upsert_tasks_batch", { tasks: expect.any(Array) });
    expect(mockedInvoke).toHaveBeenCalledWith("db_add_history_batch", { items: expect.any(Array) });
    // 校验通过后清理旧数据源
    expect(mockForageRemoveItem).toHaveBeenCalledWith("download_tasks");
    expect(localStorage.getItem("history")).toBeNull();
    // 迁移后不再弹窗
    const second = await detectLegacyData();
    expect(second.needsPrompt).toBe(false);
  });

  it("marks empty domains done silently without prompting", async () => {
    const first = await detectLegacyData();
    expect(first.needsPrompt).toBe(false);
    expect(localStorage.getItem("yt_dlp_gui_legacy_migration")).toContain('"done"');
  });

  it("throws when verification fails and keeps pending state", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    const mockedInvoke = vi.mocked(invoke);
    mockForageGetItem.mockResolvedValue([sampleTask({ id: "dl_7", status: "queued" })]);

    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "db_get_tasks") return [];
      return undefined;
    });

    await expect(migrateLegacyData({ tasks: 1, history: 0 })).rejects.toThrow();
    // 失败不标记不清理，下次启动可重试
    const ledgerRaw = localStorage.getItem("yt_dlp_gui_legacy_migration");
    expect(ledgerRaw === null || !ledgerRaw.includes('"tasks":"done"')).toBe(true);
    expect(mockForageRemoveItem).not.toHaveBeenCalled();
  });
});
