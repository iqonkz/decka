import { useSyncExternalStore } from "react";
import type { SysStats } from "@decka/shared";
import { HISTORY_LENGTH } from "./sparkline";
import { percent } from "./format";

// The last 60 readings of CPU and RAM for the graph of the `sys` widget (DATA_MODEL.md 6.5). They live outside the widget,
// so a page swipe that unmounts it does not lose them; the host clears them when the connection comes back.

export interface SysHistory { cpu: number[]; ram: number[]; }

let history: SysHistory = { cpu: [], ram: [] };
const listeners = new Set<() => void>();

export function recordSys(sys: SysStats | null): void {
  if (!sys) return;
  history = {
    cpu: [...history.cpu, sys.cpu].slice(-HISTORY_LENGTH),
    ram: [...history.ram, percent(sys.mem.usedMb, sys.mem.totalMb)].slice(-HISTORY_LENGTH),
  };
  listeners.forEach((fn) => fn());
}

export function resetSysHistory(): void {
  history = { cpu: [], ram: [] };
  listeners.forEach((fn) => fn());
}

export function useSysHistory(): SysHistory {
  return useSyncExternalStore(
    (fn) => {
      listeners.add(fn);
      return () => listeners.delete(fn);
    },
    () => history,
    () => history,
  );
}
