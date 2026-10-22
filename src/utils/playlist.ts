export const filterPlaylistEntries = <T extends { playlist_index?: number | null }>(
  entries: Array<T | null | undefined>,
): Array<T & { playlist_index: number }> =>
  entries.flatMap((entry, index) =>
    entry == null ? [] : [{ ...entry, playlist_index: entry.playlist_index ?? index + 1 }],
  );

/** A selected episode still needs playlist mode, even when only one is selected. */
export const getPlaylistDownloadOptions = (isPlaylist: boolean, selectedItems: number[]) => {
  if (!isPlaylist) return { noPlaylist: true, playlistItems: null };
  if (!selectedItems.length) return null;
  return {
    noPlaylist: false,
    playlistItems: [...new Set(selectedItems)].sort((a, b) => a - b).join(","),
  };
};
