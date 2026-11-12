/// <reference types="node" />
import { afterEach, expect, test, vi } from "vitest";
import { readFileSync, mkdtempSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { runInNewContext } from "node:vm";

// Execute the exact JavaScript injected by Rust, not a reimplementation of send().
const rust = readFileSync(new URL("../src-tauri/src/commands/collector.rs", import.meta.url), "utf8");
const source = rust.match(/const COLLECTOR_SCRIPT: &str = r#"([\s\S]*?)"#;/)?.[1];
if (!source) throw new Error("Collector injection script not found");
const text = {
  title: "采集", hint: "测试", start: "开始", stop: "停止", send: "发送",
  started: "开始", stoppedByUser: "已停止", noMore: "无新增",
  done: "共收集 {count} 条链接。", sent: "正在保存 {count} 条链接。", emptyResult: "没有链接",
};

function harness(input: string[]) {
  vi.useFakeTimers();
  let links = input;
  const blobs = new Map<string, Blob>();
  const downloads: { name: string; blob: Blob }[] = [];
  const nodes = new Map<string, ReturnType<typeof element>>();
  function element() {
    const handlers = new Map<string, () => void>();
    return {
      id: "", href: "", download: "", innerHTML: "", textContent: "", disabled: false,
      style: { cssText: "", opacity: "" },
      addEventListener(event: string, callback: () => void) { handlers.set(event, callback); },
      click() {
        if (this.disabled) return;
        if (this.download) {
          const blob = blobs.get(this.href);
          if (!blob) throw new Error("Missing export Blob");
          downloads.push({ name: this.download, blob });
        }
        handlers.get("click")?.();
      },
      remove() {},
    };
  }
  for (const id of ["ydl-collector-count", "ydl-collector-start", "ydl-collector-stop", "ydl-collector-send"]) {
    nodes.set(id, element());
  }
  class TestURL extends URL {
    static createObjectURL(blob: Blob) {
      const id = `blob:https://www.facebook.com/test-${blobs.size}`;
      blobs.set(id, blob);
      return id;
    }
    static revokeObjectURL(id: string) { blobs.delete(id); }
  }
  const document = {
    title: "Collector fixture", readyState: "complete",
    body: { scrollHeight: 1000, appendChild(node: ReturnType<typeof element>) { if (node.id) nodes.set(node.id, node); } },
    querySelectorAll: () => links.map((href) => ({ href })),
    createElement: () => element(),
    getElementById: (id: string) => nodes.get(id) ?? null,
  };
  const window: any = { scrollTo: vi.fn() };
  window.top = window;
  runInNewContext(source!.replace("__PANEL_TEXT__", JSON.stringify(text)), {
    window, document, location: { hostname: "www.facebook.com", origin: "https://www.facebook.com" },
    URL: TestURL, Blob, setTimeout, clearTimeout, console,
  });
  return { window, document, downloads, nodes, setLinks: (next: string[]) => { links = next; } };
}

afterEach(() => { vi.clearAllTimers(); vi.useRealTimers(); });

for (const count of [1, 110, 150, 999]) {
  test(`sending ${count} collected links writes every URL into the TXT Blob`, async () => {
    const urls = Array.from({ length: count }, (_, i) => `https://www.facebook.com/reel/${100000000000000 + i}/`);
    const h = harness([...urls, urls[0]]);
    h.nodes.get("ydl-collector-send")!.click();
    expect(h.window.__ydl.reels.size).toBe(count);
    expect(h.downloads).toHaveLength(1);
    expect((await h.downloads[0].blob.text()).trim().split(/\r?\n/)).toEqual(urls);
    const dir = mkdtempSync(join(tmpdir(), "ydl-collector-test-"));
    try {
      const path = join(dir, h.downloads[0].name);
      writeFileSync(path, await h.downloads[0].blob.text(), "utf8");
      expect(readFileSync(path, "utf8").trim().split(/\r?\n/)).toEqual(urls);
    } finally { rmSync(dir, { recursive: true }); }
  });
}

test("Stop preserves already collected links even after virtualized DOM removes them", async () => {
  const urls = ["https://www.facebook.com/reel/123/", "https://www.facebook.com/videos/456/"];
  const h = harness(urls);
  const running = h.window.__ydl.run({ maxScans: 500, interval: 1800, stopAfterIdle: 10 });
  expect(h.window.__ydl.reels.size).toBe(2);
  h.nodes.get("ydl-collector-stop")!.click();
  h.setLinks([]);
  h.nodes.get("ydl-collector-send")!.click();
  expect(h.downloads).toHaveLength(1);
  expect((await h.downloads[0].blob.text()).trim().split(/\r?\n/)).toEqual(urls);
  await vi.advanceTimersByTimeAsync(1900);
  await running;
  expect(h.window.__ydl.running).toBe(false);
  expect(h.window.scrollTo).toHaveBeenCalledTimes(1);
  h.nodes.get("ydl-collector-send")!.click();
  expect(h.downloads).toHaveLength(2);
  expect(await h.downloads[1].blob.text()).toBe(await h.downloads[0].blob.text());
});

test("send during collection and repeated send do not consume or clear results", async () => {
  const h = harness(["https://www.facebook.com/watch/?v=789"]);
  const running = h.window.__ydl.run({ maxScans: 1, interval: 1800 });
  h.nodes.get("ydl-collector-send")!.click();
  h.nodes.get("ydl-collector-send")!.click();
  expect(h.downloads).toHaveLength(2);
  expect(h.window.__ydl.reels.size).toBe(1);
  await vi.advanceTimersByTimeAsync(1900);
  await running;
  expect(await h.downloads[0].blob.text()).toContain("https://www.facebook.com/watch/?v=789");
});

test("no links produces no empty download", () => {
  const h = harness([]);
  h.nodes.get("ydl-collector-send")!.click();
  expect(h.downloads).toHaveLength(0);
});
