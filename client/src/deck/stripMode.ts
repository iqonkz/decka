import { useSyncExternalStore } from "react";
import type { Page, StripInfo } from "@decka/shared";

/** How the app strip behaves for one program (DATA_MODEL.md 10.5); kept on this phone only. */
export type StripMode = "auto" | "tab" | "never";

export const STRIP_KEY = "decka.strip.v1";

function safeStorage(): Storage | undefined {
  try {
    return window.localStorage;
  } catch {
    return undefined;
  }
}

const isMode = (v: unknown): v is StripMode => v === "auto" || v === "tab" || v === "never";

/** `{ "<exe>": mode }`; anything unreadable is an empty map. */
export function loadStripModes(storage: Pick<Storage, "getItem"> | undefined = safeStorage()): Record<string, StripMode> {
  try {
    const raw: unknown = JSON.parse(storage?.getItem(STRIP_KEY) ?? "{}");
    if (typeof raw !== "object" || raw === null) return {};
    return Object.fromEntries(Object.entries(raw).filter((e): e is [string, StripMode] => isMode(e[1])));
  } catch {
    return {};
  }
}

/** The mode of `app`: `auto` unless the phone remembers another one. */
export function stripModeFor(modes: Record<string, StripMode>, app: string): StripMode {
  return modes[app] ?? "auto";
}

/** The apps the user turned the strip off for (the menu lists them with `Show again`). */
export function hiddenStripApps(modes: Record<string, StripMode>): string[] {
  return Object.entries(modes).filter(([, m]) => m === "never").map(([app]) => app).sort();
}

let current = loadStripModes();
const listeners = new Set<() => void>();

export function getStripModes(): Record<string, StripMode> {
  return current;
}

/** Sets the mode of one program (`auto` is the default and is not stored) and tells the screens. */
export function setStripMode(app: string, mode: StripMode, storage: Pick<Storage, "setItem"> | undefined = safeStorage()): void {
  const next = { ...current };
  if (mode === "auto") delete next[app];
  else next[app] = mode;
  current = next;
  try {
    storage?.setItem(STRIP_KEY, JSON.stringify(next));
  } catch {
    // blocked storage: the choice lasts until the app is closed
  }
  listeners.forEach((fn) => fn());
}

export function useStripModes(): Record<string, StripMode> {
  return useSyncExternalStore(
    (fn) => {
      listeners.add(fn);
      return () => listeners.delete(fn);
    },
    getStripModes,
    getStripModes,
  );
}

/**
 * The strip as a popup-like page of plain keys: two columns in landscape, two rows in portrait, in reading order
 * (DESIGN.md 4.10). The keys are ordinary buttons of the current theme.
 */
export function stripPage(strip: StripInfo, portrait: boolean): Page {
  return {
    id: `strip-${strip.template}`,
    name: strip.name,
    items: strip.keys.map((k, i) => ({
      id: `k${i}`,
      kind: "button" as const,
      x: portrait ? Math.floor(i / 2) : i % 2,
      y: portrait ? i % 2 : Math.floor(i / 2),
      w: 1,
      h: 1,
      label: k.label,
      ...(k.icon !== undefined ? { icon: k.icon } : {}),
      actionId: k.actionId,
    })),
  };
}
