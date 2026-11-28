// T32.F: the levels of the music and of the microphone arrive 20 times a second. They go straight to what moves (the meter
// of the bar, the orb) without a new state of the island, so React draws nothing for them.
export interface Levels {
  /** 0..1 of the music of the PC. */
  music: number;
  /** 0..1 of the microphone while a request listens. */
  voice: number;
}

export interface LevelSource {
  get(): Levels;
  set(next: Levels): void;
  /** Calls `fn` after every change; returns the way to stop it. */
  subscribe(fn: () => void): () => void;
}

export function createLevels(): LevelSource {
  let current: Levels = { music: 0, voice: 0 };
  const listeners = new Set<() => void>();
  return {
    get: () => current,
    set(next) {
      current = next;
      listeners.forEach((fn) => fn());
    },
    subscribe(fn) {
      listeners.add(fn);
      return () => void listeners.delete(fn);
    },
  };
}

/** The width of the meter of the bar as a part of the whole: never thinner than 12 % while a song plays. */
export function meterScale(level: number | undefined): number {
  return level === undefined ? 0 : Math.max(0.12, Math.min(1, level));
}
