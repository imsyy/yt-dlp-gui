import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { createPinia, disposePinia } from "pinia";
import persistedState from "pinia-plugin-persistedstate";
import { createRenderer, ssrContextKey } from "vue";
import { invoke } from "@tauri-apps/api/core";
import ToolManager from "@/components/ToolManager.vue";
import { useSettingStore } from "@/stores/setting";
import { useStatusStore } from "@/stores/status";
import type { ToolStatus } from "@/types";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
vi.mock("@tauri-apps/plugin-clipboard-manager", () => ({ writeText: vi.fn() }));
vi.mock("vue-i18n", () => ({ useI18n: () => ({ t: (key: string) => key }) }));
vi.mock("@/locales", () => ({ resolveLocale: () => "en-US", setI18nLocale: vi.fn() }));
vi.mock("@/utils/format", () => ({ formatError: (error: string) => error }));

const renderer = createRenderer<Record<string, never>, Record<string, never>>({
  createElement: () => ({}),
  createText: () => ({}),
  createComment: () => ({}),
  setText: () => {},
  setElementText: () => {},
  patchProp: () => {},
  parentNode: () => null,
  nextSibling: () => null,
  insert: () => {},
  remove: () => {},
});

interface Tool {
  key: "yt-dlp";
  title: string;
  installCommand: string;
  updateCommand: string;
}

interface ManagerState {
  localPathInput: string;
  statuses: Record<string, ToolStatus | null>;
  tools: Tool[];
  getSourceOptions: (tool: string) => { value: string }[];
  handleLocalPathApply: () => Promise<void>;
  handleInstall: (tool: Tool) => Promise<void>;
  handleUpdate: (tool: Tool) => Promise<void> | undefined;
  hasToolUpdate: (tool: string) => boolean;
}

const storage = new Map<string, string>();
const activeManagers: (() => void)[] = [];
let backendConfig = { ytdlp: "local", ytdlpLocalPath: "" };
const workingStatus = (path: string, source: ToolStatus["source"] = "local"): ToolStatus => ({
  installed: true,
  runnable: true,
  version: "2026.10.22",
  path,
  source,
  isManaged: source === "managed",
  canUpdate: source === "managed",
  error: null,
});

const settle = async () => {
  for (let i = 0; i < 12; i++) await Promise.resolve();
  await nextTick();
};

const mountManager = async () => {
  const pinia = createPinia().use(persistedState);
  let state: ManagerState;
  const Component = {
    ...ToolManager,
    render: () => {
      state = (getCurrentInstance() as unknown as { setupState: ManagerState }).setupState;
      return null;
    },
  };
  const app = renderer.createApp({ render: () => h(Component) });
  app.use(pinia);
  app.provide(ssrContextKey, { modules: new Set() });
  const settings = useSettingStore(pinia);
  settings.ytdlpSource = "local";
  app.mount({});
  activeManagers.push(() => {
    app.unmount();
    disposePinia(pinia);
  });
  await settle();
  return { state: state!, settings, status: useStatusStore(pinia) };
};

beforeEach(() => {
  storage.clear();
  backendConfig = { ytdlp: "local", ytdlpLocalPath: "" };
  const localStorage = {
    getItem: (key: string) => storage.get(key) ?? null,
    setItem: (key: string, value: string) => storage.set(key, value),
  };
  vi.stubGlobal("localStorage", localStorage);
  vi.stubGlobal("window", { localStorage, $message: { error: vi.fn(), success: vi.fn() } });
  vi.mocked(invoke).mockReset();
  vi.mocked(invoke).mockImplementation(async (command, args) => {
    if (command === "set_tool_sources") {
      backendConfig = args as typeof backendConfig;
    } else if (command === "get_ytdlp_status") {
      return backendConfig.ytdlpLocalPath
        ? workingStatus(backendConfig.ytdlpLocalPath)
        : {
            ...workingStatus(""),
            installed: false,
            runnable: false,
            error: "err_ytdlp_local_path_empty",
          };
    } else if (command.startsWith("get_")) {
      return workingStatus("system-tool", "system");
    }
  });
});

afterEach(() => {
  activeManagers.splice(0).forEach((unmount) => unmount());
  vi.unstubAllGlobals();
});

test("saves the exact local path and keeps an empty saved path visibly invalid", async () => {
  const manager = await mountManager();
  expect(manager.state.getSourceOptions("yt-dlp").map((option) => option.value)).toContain("local");
  expect(manager.state.getSourceOptions("deno").map((option) => option.value)).not.toContain(
    "local",
  );
  const path = " C:\\下载文件\\my venv\\Scripts\\yt-dlp.exe ";
  manager.state.localPathInput = path;
  await manager.state.handleLocalPathApply();
  await nextTick();
  expect(manager.settings.ytdlpLocalPath).toBe(path);
  expect(backendConfig.ytdlpLocalPath).toBe(path);
  expect(JSON.parse(storage.get("setting")!).ytdlpLocalPath).toBe(path);

  manager.state.localPathInput = "";
  await manager.state.handleLocalPathApply();
  expect(manager.settings.ytdlpSource).toBe("local");
  expect(manager.settings.ytdlpLocalPath).toBe("");
  expect(manager.state.localPathInput).toBe("");
  expect(manager.state.statuses["yt-dlp"]?.runnable).toBe(false);
  expect(manager.state.statuses["yt-dlp"]?.error).toBe("err_ytdlp_local_path_empty");
  expect(backendConfig.ytdlp).toBe("local");
});

test("local and CLI installations cannot trigger download or update operations", async () => {
  const manager = await mountManager();
  const tool = manager.state.tools[0];
  await manager.state.handleInstall(tool);
  await manager.state.handleUpdate(tool);
  manager.status.toolUpdates["yt-dlp"] = {
    updateAvailable: true,
    currentVersion: "old",
    latestVersion: "new",
  };
  expect(manager.state.hasToolUpdate("yt-dlp")).toBe(false);

  manager.state.statuses["yt-dlp"] = workingStatus("CLI-path", "custom");
  manager.settings.ytdlpSource = "managed";
  await manager.state.handleInstall(tool);
  await manager.state.handleUpdate(tool);
  manager.state.localPathInput = "unapplied-edit";
  await manager.state.handleLocalPathApply();
  expect(manager.settings.ytdlpLocalPath).toBe("");
  expect(invoke).not.toHaveBeenCalledWith("download_ytdlp");
  expect(invoke).not.toHaveBeenCalledWith("update_ytdlp");
});

test("an old executable probe cannot replace the status of a newly saved path", async () => {
  let finishOldProbe: (status: ToolStatus) => void = () => {};
  const invokeNormally = vi.mocked(invoke).getMockImplementation()!;
  let first = true;
  vi.mocked(invoke).mockImplementation((command, args) => {
    if (command === "get_ytdlp_status" && first) {
      first = false;
      return new Promise((resolve) => {
        finishOldProbe = resolve;
      });
    }
    return invokeNormally(command, args);
  });
  const manager = await mountManager();
  manager.state.localPathInput = "C:\\new venv\\Scripts\\yt-dlp.exe";
  await manager.state.handleLocalPathApply();
  finishOldProbe(workingStatus("old-managed-path", "managed"));
  await settle();
  expect(manager.state.statuses["yt-dlp"]?.path).toBe("C:\\new venv\\Scripts\\yt-dlp.exe");
  expect(manager.state.statuses["yt-dlp"]?.source).toBe("local");
});
