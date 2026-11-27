// The settings drafts of M32 were kept in the window's localStorage - among them the ICS link, which is a secret. They live
// in the agent now (the link under DPAPI): whatever is left is removed at every start.
export const M32_STORAGE_KEYS = ["decka.calendar.v1", "decka.wakeword.v1", "decka.tray.favorites.v1"] as const;

export function removeM32Storage(storage: Pick<Storage, "removeItem">): void {
  for (const key of M32_STORAGE_KEYS) storage.removeItem(key);
}
