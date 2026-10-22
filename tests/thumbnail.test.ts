import { beforeEach, expect, test, vi } from "vitest";
import { effectScope, nextTick, reactive, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useThumbnail } from "@/composables/useThumbnail";

const { settings } = vi.hoisted(() => ({ settings: { proxy: "" } }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@/stores/setting", () => ({ useSettingStore: () => reactive(settings) }));

beforeEach(() => {
  vi.clearAllMocks();
  settings.proxy = "";
});

test("uses direct image URLs without a proxy", () => {
  const scope = effectScope();
  const thumbnail = scope.run(() => useThumbnail("https://example.com/cover.jpg"))!;
  expect(thumbnail.src.value).toBe("https://example.com/cover.jpg");
  expect(invoke).not.toHaveBeenCalled();
  scope.stop();
});

test("passes SOCKS settings to Rust and revokes the binary image URL on disposal", async () => {
  settings.proxy = "socks5://127.0.0.1:1080";
  vi.mocked(invoke).mockResolvedValue(new Uint8Array([1, 2, 3]).buffer);
  const revoke = vi.spyOn(URL, "revokeObjectURL");
  const scope = effectScope();
  const thumbnail = scope.run(() => useThumbnail("https://example.com/cover.jpg"))!;
  await vi.waitFor(() => expect(thumbnail.src.value).toMatch(/^blob:/));
  expect(invoke).toHaveBeenCalledWith("fetch_thumbnail", {
    url: "https://example.com/cover.jpg",
    proxy: settings.proxy,
  });
  const objectUrl = thumbnail.src.value;
  scope.stop();
  expect(revoke).toHaveBeenCalledWith(objectUrl);
  revoke.mockRestore();
});

test("discards a stale response after the image URL changes", async () => {
  settings.proxy = "socks5://127.0.0.1:1080";
  let resolveFirst!: (bytes: ArrayBuffer) => void;
  vi.mocked(invoke).mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        resolveFirst = resolve;
      }),
  );
  vi.mocked(invoke).mockResolvedValueOnce(new Uint8Array([4]).buffer);
  const url = ref("https://example.com/first.jpg");
  const scope = effectScope();
  const thumbnail = scope.run(() => useThumbnail(url))!;
  url.value = "https://example.com/second.jpg";
  await vi.waitFor(() => expect(thumbnail.src.value).toMatch(/^blob:/));
  const secondUrl = thumbnail.src.value;
  resolveFirst(new Uint8Array([1]).buffer);
  await nextTick();
  await nextTick();
  expect(thumbnail.src.value).toBe(secondUrl);
  scope.stop();
});
