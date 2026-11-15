// Glass runs heavy effects; on a slow device it switches to a lighter look ("reduced", DESIGN.md 1.4).

export const REDUCED_KEY = "decka.glass.reduced.v1";
/// Below this mean frame rate the Glass theme goes to `reduced`.
export const MIN_FPS = 45;
export const MEASURE_MS = 3000;

/** Mean frames per second from frame timestamps (ms); fewer than two frames -> null. */
export function averageFps(timestamps: number[]): number | null {
  if (timestamps.length < 2) return null;
  const first = timestamps[0] as number;
  const last = timestamps[timestamps.length - 1] as number;
  if (last <= first) return null;
  return ((timestamps.length - 1) * 1000) / (last - first);
}

export function shouldReduce(fps: number | null): boolean {
  return fps !== null && fps < MIN_FPS;
}

/** The stored decision: `true` / `false`, or `null` when none was made yet. Storage may be unavailable. */
export function loadReduced(storage: Pick<Storage, "getItem"> | undefined = safeStorage()): boolean | null {
  try {
    const v = storage?.getItem(REDUCED_KEY);
    return v === "1" ? true : v === "0" ? false : null;
  } catch {
    return null;
  }
}

export function saveReduced(reduced: boolean, storage: Pick<Storage, "setItem"> | undefined = safeStorage()): void {
  try {
    storage?.setItem(REDUCED_KEY, reduced ? "1" : "0");
  } catch {
    // private window or blocked storage: the decision just is not remembered
  }
}

function safeStorage(): Storage | undefined {
  try {
    return window.localStorage;
  } catch {
    return undefined;
  }
}

/** Counts frames for `ms` and resolves with the mean frame rate (null if the page was hidden). */
export function measureFps(ms: number = MEASURE_MS): Promise<number | null> {
  return new Promise((resolve) => {
    const times: number[] = [];
    const start = performance.now();
    const tick = (t: number) => {
      times.push(t);
      if (t - start < ms) requestAnimationFrame(tick);
      else resolve(averageFps(times));
    };
    requestAnimationFrame(tick);
  });
}
