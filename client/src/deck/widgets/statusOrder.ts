import type { StatusItem, StatusState } from "@decka/shared";

/** State and level colors (DATA_MODEL.md 6). */
export const STATE_COLOR: Record<StatusState, string> = {
  ok: "#16a34a",
  warn: "#d97706",
  error: "#dc2626",
  running: "#2563eb",
  unknown: "#6b7280",
};

const ORDER: Record<StatusState, number> = { error: 0, warn: 1, running: 2, unknown: 3, ok: 4 };

/** Optionally filtered by group; worst states first, then by label. */
export function sortStatuses(items: StatusItem[], group?: string): StatusItem[] {
  const filtered = group === undefined ? items : items.filter((s) => s.group === group);
  return [...filtered].sort((a, b) => ORDER[a.state] - ORDER[b.state] || a.label.localeCompare(b.label));
}
