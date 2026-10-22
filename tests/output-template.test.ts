import { test, expect } from "vitest";
import { composeOutputTemplate, DEFAULT_OUTPUT_TEMPLATE } from "@/utils/output-template";

test("places static prefix and suffix around the filename before its extension", () => {
  expect(composeOutputTemplate("%(upload_date)s - %(title)s.%(ext)s", "[Archive] ", " [4K]")).toBe(
    "[Archive] %(upload_date)s - %(title)s [4K].%(ext)s",
  );
});

test("falls back to the shared default template", () => {
  expect(composeOutputTemplate("", "", "")).toBe(DEFAULT_OUTPUT_TEMPLATE);
});

test("puts each playlist in its own directory with a title or ID fallback", () => {
  expect(composeOutputTemplate("%(title)s.%(ext)s", "", "", true)).toBe(
    "%(playlist_title,playlist_id|Playlist)s/%(title)s.%(ext)s",
  );
});

test("applies static affixes to the filename within custom directories", () => {
  expect(composeOutputTemplate("%(uploader)s/%(title)s.%(ext)s", "[Video] ", " [HD]", true)).toBe(
    "%(uploader)s/%(playlist_title,playlist_id|Playlist)s/[Video] %(title)s [HD].%(ext)s",
  );
  expect(composeOutputTemplate("Archive\\%(title)s.%(ext)s", "[Video] ", "")).toBe(
    "Archive\\[Video] %(title)s.%(ext)s",
  );
});

test("keeps separators inside yt-dlp date expressions intact", () => {
  expect(composeOutputTemplate("%(upload_date>%Y/%m)s %(title)s.%(ext)s", "[Video] ", "")).toBe(
    "[Video] %(upload_date>%Y/%m)s %(title)s.%(ext)s",
  );
});

test("preserves absolute custom template directories when grouping playlists", () => {
  expect(composeOutputTemplate("C:\\Videos\\%(title)s.%(ext)s", "", "", true)).toBe(
    "C:\\Videos\\%(playlist_title,playlist_id|Playlist)s/%(title)s.%(ext)s",
  );
});
