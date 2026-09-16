import { test, expect } from "vitest";
import { normalizeDeepLinkVideoUrl } from "@/utils/url";

test("removes playlist context from a YouTube watch URL", () => {
  expect(
    normalizeDeepLinkVideoUrl(
      "https://www.youtube.com/watch?v=gRX3Gm-YPRY&list=PLF06D437EE0D16A9F&index=6&t=15",
    ),
  ).toBe("https://www.youtube.com/watch?v=gRX3Gm-YPRY&t=15");
});

test("removes playlist context from a youtu.be video URL", () => {
  expect(
    normalizeDeepLinkVideoUrl("https://youtu.be/gRX3Gm-YPRY?list=PL123&index=2"),
  ).toBe("https://youtu.be/gRX3Gm-YPRY");
});

test("preserves an explicit YouTube playlist URL", () => {
  const playlistUrl = "https://www.youtube.com/playlist?list=PLF06D437EE0D16A9F";
  expect(normalizeDeepLinkVideoUrl(playlistUrl)).toBe(playlistUrl);
});

test("preserves non-YouTube URLs", () => {
  const videoUrl = "https://www.bilibili.com/video/BV123?list=example";
  expect(normalizeDeepLinkVideoUrl(videoUrl)).toBe(videoUrl);
});
