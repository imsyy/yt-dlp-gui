import assert from "node:assert/strict";
import test from "node:test";
import { normalizeDeepLinkVideoUrl } from "../src/utils/url.ts";

test("removes playlist context from a YouTube watch URL", () => {
  assert.equal(
    normalizeDeepLinkVideoUrl(
      "https://www.youtube.com/watch?v=gRX3Gm-YPRY&list=PLF06D437EE0D16A9F&index=6&t=15",
    ),
    "https://www.youtube.com/watch?v=gRX3Gm-YPRY&t=15",
  );
});

test("removes playlist context from a youtu.be video URL", () => {
  assert.equal(
    normalizeDeepLinkVideoUrl("https://youtu.be/gRX3Gm-YPRY?list=PL123&index=2"),
    "https://youtu.be/gRX3Gm-YPRY",
  );
});

test("preserves an explicit YouTube playlist URL", () => {
  const playlistUrl = "https://www.youtube.com/playlist?list=PLF06D437EE0D16A9F";
  assert.equal(normalizeDeepLinkVideoUrl(playlistUrl), playlistUrl);
});

test("preserves non-YouTube URLs", () => {
  const videoUrl = "https://www.bilibili.com/video/BV123?list=example";
  assert.equal(normalizeDeepLinkVideoUrl(videoUrl), videoUrl);
});
