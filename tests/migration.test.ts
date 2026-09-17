import { describe, expect, it, vi, beforeEach } from "vitest";
import { normalizeTaskParams, migrateLegacyHistory } from "@/utils/migration";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
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

describe("migrateLegacyHistory", () => {
  beforeEach(() => {
    localStorageMock.clear();
    vi.clearAllMocks();
  });

  it("migrates object-style history from localStorage and cleans old key", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    const mockLegacyHistory = {
      items: [
        { url: "https://example.com/v1", title: "Video 1", time: 1000 },
        { url: "https://example.com/v2", title: "Video 2", time: 2000 },
      ],
    };
    localStorage.setItem("history", JSON.stringify(mockLegacyHistory));

    const result = await migrateLegacyHistory();

    expect(result).toHaveLength(2);
    expect(result?.[0].url).toBe("https://example.com/v1");
    expect(invoke).toHaveBeenCalledWith("db_add_history_batch", {
      items: mockLegacyHistory.items,
    });
    expect(localStorage.getItem("history")).toBeNull();
    expect(localStorage.getItem("yt_dlp_gui_history_migrated_v1")).toBe("true");
  });

  it("migrates string array-style legacy history gracefully", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    const mockUrls = ["https://example.com/a", "https://example.com/b"];
    localStorage.setItem("history", JSON.stringify(mockUrls));

    const result = await migrateLegacyHistory();

    expect(result).toHaveLength(2);
    expect(result?.[0].title).toBe("https://example.com/a");
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(localStorage.getItem("history")).toBeNull();
  });

  it("skips migration if flag is already set", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    localStorage.setItem("yt_dlp_gui_history_migrated_v1", "true");
    localStorage.setItem("history", JSON.stringify({ items: [] }));

    const result = await migrateLegacyHistory();

    expect(result).toBeNull();
    expect(invoke).not.toHaveBeenCalled();
  });
});
