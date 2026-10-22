import { test, expect } from "vitest";
import { filterPlaylistEntries, getPlaylistDownloadOptions } from "@/utils/playlist";

test("filterPlaylistEntries removes unavailable playlist entries", () => {
  const availableEntry = { id: "video-1", title: "Available video", playlist_index: 2 };

  expect(filterPlaylistEntries([null, availableEntry, undefined])).toEqual([availableEntry]);
});

test("preserves original positions when unavailable entries have no metadata", () => {
  expect(
    filterPlaylistEntries([
      null,
      { id: "second", playlist_index: undefined },
      null,
      { id: "fourth" },
    ]),
  ).toEqual([
    { id: "second", playlist_index: 2 },
    { id: "fourth", playlist_index: 4 },
  ]);
});

test("preserves extractor playlist indices rather than renumbering a subset", () => {
  expect(filterPlaylistEntries([{ playlist_index: 12 }, { playlist_index: 19 }])).toEqual([
    { playlist_index: 12 },
    { playlist_index: 19 },
  ]);
});

test("selecting one episode keeps playlist mode and its exact index", () => {
  expect(getPlaylistDownloadOptions(true, [7])).toEqual({ noPlaylist: false, playlistItems: "7" });
});

test("deselecting every episode does not silently download the whole playlist", () => {
  expect(getPlaylistDownloadOptions(true, [])).toBeNull();
});

test("sorts and deduplicates selections without mutating UI state", () => {
  const selected = [3, 1, 3];
  expect(getPlaylistDownloadOptions(true, selected)).toEqual({
    noPlaylist: false,
    playlistItems: "1,3",
  });
  expect(selected).toEqual([3, 1, 3]);
  expect(getPlaylistDownloadOptions(false, [])).toEqual({ noPlaylist: true, playlistItems: null });
});
