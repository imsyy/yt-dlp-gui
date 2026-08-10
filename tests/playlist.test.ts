import assert from "node:assert/strict";
import test from "node:test";
import { filterPlaylistEntries } from "../src/utils/playlist.ts";

test("filterPlaylistEntries removes unavailable playlist entries", () => {
  const availableEntry = { id: "video-1", title: "Available video" };

  assert.deepEqual(
    filterPlaylistEntries([null, availableEntry, undefined]),
    [availableEntry],
  );
});
