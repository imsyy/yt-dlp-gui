import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { createPinia, disposePinia, setActivePinia } from "pinia";
import { invoke } from "@tauri-apps/api/core";
import { useDownloadStore } from "@/stores/download";
import { normalizeTaskParams } from "@/utils/taskParams";
import { MAX_TASK_LOGS } from "@/utils/logs";
import type { DownloadTask } from "@/types";

const { handlers, mockState } = vi.hoisted(() => ({
  handlers: new Map<string, (event: { payload: Record<string, unknown> }) => void>(),
  mockState: {
    settings: { notifyMode: "none", showTaskbarProgress: false, maxConcurrent: 3 },
  },
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
  useSettingStore: () => mockState.settings,
}));

let pinia: ReturnType<typeof createPinia>;
beforeEach(() => {
  pinia = createPinia();
  setActivePinia(pinia);
  mockState.settings = reactive({
    notifyMode: "none",
    showTaskbarProgress: false,
    maxConcurrent: 3,
  });
  handlers.clear();
  vi.clearAllMocks();
  vi.mocked(invoke).mockReset();
  vi.mocked(invoke).mockImplementation(async (command) =>
    command === "db_get_tasks" ? [] : undefined,
  );
});

afterEach(() => disposePinia(pinia));

const createTask = (id: string, status: DownloadTask["status"] = "queued"): DownloadTask => ({
  id,
  url: "https://example.com/video",
  title: "Video",
  thumbnail: "",
  formatLabel: "",
  status,
  percent: 0,
  speed: "",
  eta: "",
  downloaded: "",
  total: "",
  logs: [],
  createdAt: 0,
  params: normalizeTaskParams(),
});

const deferred = <T = void>() => {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((accept, fail) => {
    resolve = accept;
    reject = fail;
  });
  return { promise, resolve, reject };
};

const readyStore = async () => {
  const store = useDownloadStore();
  await vi.waitFor(() => {
    expect(store.loaded).toBe(true);
    expect(handlers.has("download-error")).toBe(true);
  });
  return store;
};

const starts = () =>
  vi.mocked(invoke).mock.calls.filter(([command]) => command === "start_download");

const complete = (id: string) =>
  handlers.get("download-complete")!({ payload: { id, outputFile: `${id}.mp4` } });

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

test("restored queues fill every available slot and resume after a completion", async () => {
  mockState.settings.maxConcurrent = 2;
  vi.mocked(invoke).mockImplementation(async (command) =>
    command === "db_get_tasks"
      ? [createTask("first"), createTask("second"), createTask("third")]
      : undefined,
  );
  const store = await readyStore();
  await vi.waitFor(() => expect(starts()).toHaveLength(2));
  expect(store.activeCount).toBe(2);
  expect(store.tasks.find((task) => task.id === "third")?.status).toBe("queued");

  complete("first");
  await vi.waitFor(() => expect(starts()).toHaveLength(3));
  expect(store.activeCount).toBe(2);
});

test("a failed start keeps its diagnostics and does not stall the remaining queue", async () => {
  mockState.settings.maxConcurrent = 2;
  vi.mocked(invoke).mockImplementation(async (command, args) => {
    if (command === "db_get_tasks") {
      return [createTask("broken"), createTask("second"), createTask("third")];
    }
    if (
      command === "start_download" &&
      (args as { params: { id: string } }).params.id === "broken"
    ) {
      throw new Error("cannot spawn yt-dlp");
    }
  });
  const store = await readyStore();
  await vi.waitFor(() => expect(starts()).toHaveLength(3));
  expect(store.tasks[0]).toMatchObject({ status: "error", error: "cannot spawn yt-dlp" });
  expect(store.activeCount).toBe(2);
  expect(invoke).toHaveBeenCalledWith(
    "db_upsert_task",
    expect.objectContaining({ task: expect.objectContaining({ id: "broken", status: "error" }) }),
  );
});

test("concurrent submissions start once each without exceeding the configured limit", async () => {
  mockState.settings.maxConcurrent = 2;
  const store = await readyStore();
  await Promise.all([
    store.addTask(createTask("one")),
    store.addTask(createTask("two")),
    store.addTask(createTask("three")),
  ]);
  expect(starts()).toHaveLength(2);
  expect(
    new Set(starts().map(([, args]) => (args as { params: { id: string } }).params.id)).size,
  ).toBe(2);
  expect(store.activeCount).toBe(2);
  expect(store.tasks.filter((task) => task.status === "queued")).toHaveLength(1);

  mockState.settings.maxConcurrent = 3;
  await vi.waitFor(() => expect(starts()).toHaveLength(3));
  expect(store.activeCount).toBe(3);
});

test("an unpersisted task cannot be picked up by another scheduling trigger", async () => {
  const store = await readyStore();
  const writing = deferred();
  vi.mocked(invoke).mockImplementation(async (command) => {
    if (command === "db_upsert_task") await writing.promise;
  });
  const adding = store.addTask(createTask("pending"));
  await vi.waitFor(() => expect(store.tasks).toHaveLength(1));
  mockState.settings.maxConcurrent = 4;
  await nextTick();
  expect(starts()).toHaveLength(0);

  writing.resolve();
  expect(await adding).toBe("started");
  expect(starts()).toHaveLength(1);
});

test("a database failure prevents the task from starting", async () => {
  const store = await readyStore();
  const error = vi.spyOn(console, "error").mockImplementation(() => {});
  vi.mocked(invoke).mockRejectedValue(new Error("database unavailable"));
  expect(await store.addTask(createTask("unsaved"))).toBe("failed");
  expect(store.tasks[0].status).toBe("error");
  expect(starts()).toHaveLength(0);
  error.mockRestore();
});

test("cancelling during startup waits for the process and keeps its slot reserved", async () => {
  mockState.settings.maxConcurrent = 1;
  const store = await readyStore();
  const starting = deferred();
  const cancelling = deferred();
  vi.mocked(invoke).mockImplementation(async (command) => {
    if (command === "start_download" && starts().length === 1) await starting.promise;
    if (command === "cancel_download") await cancelling.promise;
  });
  const addingFirst = store.addTask(createTask("first"));
  await vi.waitFor(() => expect(starts()).toHaveLength(1));
  const cancelled = store.cancelTask("first");
  const addingSecond = store.addTask(createTask("second"));
  expect(invoke).not.toHaveBeenCalledWith("cancel_download", expect.anything());
  expect(store.activeCount).toBe(1);
  complete("first");
  expect(store.tasks.find((task) => task.id === "first")?.status).toBe("cancelled");

  starting.resolve();
  await vi.waitFor(() =>
    expect(invoke).toHaveBeenCalledWith("cancel_download", { id: "first", deleteFiles: false }),
  );
  expect(starts()).toHaveLength(1);
  expect(store.activeCount).toBe(1);
  cancelling.resolve();
  await cancelled;
  await Promise.all([addingFirst, addingSecond]);
  await vi.waitFor(() => expect(starts()).toHaveLength(2));
  expect(store.tasks.find((task) => task.id === "first")?.status).toBe("cancelled");
});

test("a startup rejection cannot turn a cancelled task back into an error", async () => {
  const store = await readyStore();
  const starting = deferred();
  vi.mocked(invoke).mockImplementation(async (command) => {
    if (command === "start_download") await starting.promise;
  });
  const adding = store.addTask(createTask("first"));
  await vi.waitFor(() => expect(starts()).toHaveLength(1));
  const cancelled = store.cancelTask("first");
  starting.reject(new Error("FFmpeg unavailable"));
  await Promise.all([adding, cancelled]);
  expect(store.tasks[0].status).toBe("cancelled");
  expect(store.tasks[0].error).toBeUndefined();
});

test("retries are deduplicated and cancellation during their persistence prevents startup", async () => {
  const store = await readyStore();
  store.tasks.push(createTask("failed", "error"));
  const deleting = deferred();
  vi.mocked(invoke).mockImplementation(async (command) => {
    if (command === "db_delete_task") await deleting.promise;
  });
  const retrying = store.retryTask("failed");
  const replacementId = store.tasks[0].id;
  await store.retryTask("failed");
  const cancelling = store.cancelTask(replacementId);
  await store.retryTask(replacementId);
  expect(
    vi.mocked(invoke).mock.calls.filter(([command]) => command === "db_delete_task"),
  ).toHaveLength(1);
  deleting.resolve();
  await Promise.all([retrying, cancelling]);
  expect(store.tasks).toHaveLength(1);
  expect(store.tasks[0].status).toBe("cancelled");
  expect(starts()).toHaveLength(0);
});

test("preparing replacements are kept out of the queue until their new row is saved", async () => {
  const store = await readyStore();
  await store.addTask(createTask("preparing", "preparing"));
  const deleting = deferred();
  vi.mocked(invoke).mockImplementation(async (command) => {
    if (command === "db_delete_task") await deleting.promise;
  });
  const replacing = store.addTask(createTask("ready"), "preparing");
  await vi.waitFor(() => expect(store.tasks[0].id).toBe("ready"));
  mockState.settings.maxConcurrent = 4;
  await nextTick();
  expect(starts()).toHaveLength(0);
  deleting.resolve();
  expect(await replacing).toBe("started");
  expect(store.tasks).toHaveLength(1);
  expect(starts()).toHaveLength(1);
});

test("removing a task waits for older writes and cannot recreate an orphaned row", async () => {
  const store = await readyStore();
  const writing = deferred();
  let saved = false;
  vi.mocked(invoke).mockImplementation(async (command) => {
    if (command === "db_upsert_task" && !saved) {
      await writing.promise;
      saved = true;
    }
    if (command === "db_delete_task") expect(saved).toBe(true);
  });
  const adding = store.addTask(createTask("remove"));
  await vi.waitFor(() => expect(store.tasks).toHaveLength(1));
  const removing = store.removeTask("remove");
  writing.resolve();
  await Promise.all([adding, removing]);
  expect(store.tasks).toHaveLength(0);
  expect(starts()).toHaveLength(0);
  expect(invoke).toHaveBeenLastCalledWith("db_delete_task", { id: "remove" });
});

test("re-downloading preserves completed history and sends its clone through the queue", async () => {
  const store = await readyStore();
  store.tasks.push(createTask("history", "completed"));
  await store.reDownloadTask("history");
  expect(store.tasks).toHaveLength(2);
  expect(store.tasks.find((task) => task.id === "history")?.status).toBe("completed");
  expect(store.tasks[0].status).toBe("downloading");
  expect(starts()).toHaveLength(1);
});

test("clearing cancelled tasks waits for in-flight startup and cannot recreate their rows", async () => {
  const store = await readyStore();
  const starting = deferred();
  vi.mocked(invoke).mockImplementation(async (command) => {
    if (command === "start_download") await starting.promise;
  });
  const adding = store.addTask(createTask("clear"));
  await vi.waitFor(() => expect(starts()).toHaveLength(1));
  const cancelling = store.cancelTask("clear");
  const clearing = store.clearCancelled();
  expect(store.tasks).toHaveLength(0);
  expect(invoke).not.toHaveBeenCalledWith("db_delete_tasks", expect.anything());
  starting.resolve();
  await Promise.all([adding, cancelling, clearing]);
  expect(invoke).toHaveBeenLastCalledWith("db_delete_tasks", { ids: ["clear"] });
});

test("clearing completed history waits for the final state to be persisted", async () => {
  const store = await readyStore();
  store.tasks.push(createTask("done", "downloading"));
  const writing = deferred();
  vi.mocked(invoke).mockImplementation(async (command) => {
    if (command === "db_upsert_task") await writing.promise;
  });
  complete("done");
  const clearing = store.clearCompleted();
  expect(store.tasks).toHaveLength(0);
  expect(invoke).not.toHaveBeenCalledWith("db_delete_tasks", expect.anything());
  writing.resolve();
  await clearing;
  expect(invoke).toHaveBeenLastCalledWith("db_delete_tasks", { ids: ["done"] });
});
