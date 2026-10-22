import { beforeEach, expect, test, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { invoke } from "@tauri-apps/api/core";
import { useDownloadStore } from "@/stores/download";
import { normalizeTaskParams } from "@/utils/taskParams";
import { MAX_TASK_LOGS } from "@/utils/logs";

const { handlers } = vi.hoisted(() => ({
  handlers: new Map<string, (event: { payload: Record<string, unknown> }) => void>(),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (event, handler) => {
    handlers.set(event, handler);
    return vi.fn();
  }),
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(async () => []) }));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ setProgressBar: vi.fn() }),
  ProgressBarStatus: { None: "none", Normal: "normal" },
}));
vi.mock("@tauri-apps/plugin-notification", () => ({
  isPermissionGranted: vi.fn(),
  requestPermission: vi.fn(),
  sendNotification: vi.fn(),
}));
vi.mock("@/locales", () => ({ default: { global: { t: (key: string) => key } } }));
vi.mock("@/stores/setting", () => ({
  useSettingStore: () => ({ notifyMode: "none", showTaskbarProgress: false, maxConcurrent: 3 }),
}));

beforeEach(() => {
  setActivePinia(createPinia());
  handlers.clear();
  vi.clearAllMocks();
});

test("completed downloads retain and persist their bounded diagnostic logs", async () => {
  const store = useDownloadStore();
  await vi.waitFor(() => expect(handlers.has("download-complete")).toBe(true));
  store.tasks.push({
    id: "test",
    url: "https://example.com/video",
    title: "Video",
    thumbnail: "",
    formatLabel: "",
    status: "downloading",
    percent: 0,
    speed: "",
    eta: "",
    downloaded: "",
    total: "",
    logs: [],
    createdAt: 0,
    params: normalizeTaskParams(),
  });
  for (let index = 0; index < 600; index += 1) {
    handlers.get("download-log")!({ payload: { id: "test", line: `line ${index}` } });
  }
  handlers.get("download-complete")!({ payload: { id: "test", outputFile: "video.mp4" } });
  const task = store.tasks[0];
  expect(task.status).toBe("completed");
  expect(task.logs).toHaveLength(MAX_TASK_LOGS);
  expect(task.logs[task.logs.length - 1]).toBe("line 599");
  expect(invoke).toHaveBeenCalledWith("db_upsert_task", { task });
});
