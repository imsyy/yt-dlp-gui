import { test, expect } from "vitest";
import { filterPlaylistEntries } from "@/utils/playlist";

test("filterPlaylistEntries removes unavailable playlist entries", () => {
  const availableEntry = { id: "video-1", title: "Available video" };

  expect(
    filterPlaylistEntries([null, availableEntry, undefined]),
  ).toEqual([availableEntry]);
});
