import { beforeEach, expect, test, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { invoke } from "@tauri-apps/api/core";
import { useVideoStore } from "@/stores/video";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@/stores/setting", () => ({
  useSettingStore: () => ({
    cookieMode: "none",
    proxy: "",
    defaultCustomArgs: "--compat-opt no-certifi --sleep-requests 2",
  }),
}));
vi.mock("@/locales", () => ({ default: { global: { t: (key: string) => key } } }));
vi.mock("@/utils/format", () => ({ showErrorDialog: vi.fn() }));

beforeEach(() => {
  setActivePinia(createPinia());
  vi.mocked(invoke).mockReset();
  vi.mocked(invoke).mockResolvedValue({ title: "Video", formats: [] });
});

test("applies global custom arguments when fetching metadata", async () => {
  await useVideoStore().fetchVideoInfo("https://example.com/video");
  expect(invoke).toHaveBeenCalledWith(
    "fetch_video_info",
    expect.objectContaining({
      customArgs: "--compat-opt no-certifi --sleep-requests 2",
    }),
  );
});

test("uses task or quick arguments for metadata refresh, including an explicit empty value", async () => {
  const store = useVideoStore();
  await store.fetchVideoInfo("https://example.com/video", { customArgs: "--sleep-requests 5" });
  expect(invoke).toHaveBeenLastCalledWith(
    "fetch_video_info",
    expect.objectContaining({
      customArgs: "--sleep-requests 5",
    }),
  );
  await store.fetchVideoInfo("https://example.com/video", { customArgs: "" });
  expect(invoke).toHaveBeenLastCalledWith(
    "fetch_video_info",
    expect.objectContaining({ customArgs: "" }),
  );
});
