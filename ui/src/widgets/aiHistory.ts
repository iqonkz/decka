import { useSyncExternalStore } from "react";
import { pushBurnSample, type AiInfo, type BurnSample } from "@decka/shared";

// The readings of every limit window of the last hour, for the forecast of the `ai` widget (DATA_MODEL.md 6.8). They live outside
// the widget, so a page swipe does not lose them; the host clears them when the connection comes back.

export type AiHistory = Record<string, BurnSample[]>;

let history: AiHistory = {};
const listeners = new Set<() => void>();

/** The key of a window in the history: `claude:5h`, `codex:7d`. */
export const burnKey = (provider: "claude" | "codex", label: string): string => `${provider}:${label}`;

export function recordAi(ai: AiInfo | null | undefined, now: number = Date.now()): void {
  if (!ai) return;
  const next: AiHistory = {};
  for (const provider of ["claude", "codex"] as const) {
    for (const w of ai[provider]?.windows ?? []) {
      const key = burnKey(provider, w.label);
      next[key] = pushBurnSample(history[key] ?? [], { ts: now, pct: w.pct });
    }
  }
  history = next;
  listeners.forEach((fn) => fn());
}

export function resetAiHistory(): void {
  history = {};
  listeners.forEach((fn) => fn());
}

export function useAiHistory(): AiHistory {
  return useSyncExternalStore(
    (fn) => {
      listeners.add(fn);
      return () => listeners.delete(fn);
    },
    () => history,
    () => history,
  );
}
