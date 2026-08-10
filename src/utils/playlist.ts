export const filterPlaylistEntries = <T>(entries: Array<T | null | undefined>): T[] =>
  entries.filter((entry): entry is T => entry != null);
