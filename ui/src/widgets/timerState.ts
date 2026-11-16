// The timer widget lives on the phone only (DATA_MODEL.md 6.7): a pure state machine, the clock comes in as `now`.

export interface TimerState {
  /** The chosen length, minutes. */
  minutes: number;
  remainingMs: number;
  running: boolean;
  /** When the running timer ends (ms since the epoch); null when it is not running. */
  endsAt: number | null;
  /** It ran down to zero and has not been started or reset since. */
  done: boolean;
}

export type TimerAction =
  | { type: "toggle"; now: number }      // tap: start, pause, resume (after `done`: start again)
  | { type: "reset" }                    // hold 0.8 s
  | { type: "preset"; minutes: number }  // a preset key: choose a length (and reset)
  | { type: "tick"; now: number };

export const DEFAULT_PRESETS = [25, 5];

export function timerInit(minutes: number): TimerState {
  return { minutes, remainingMs: minutes * 60_000, running: false, endsAt: null, done: false };
}

export function timerReducer(state: TimerState, action: TimerAction): TimerState {
  switch (action.type) {
    case "reset":
      return timerInit(state.minutes);
    case "preset":
      return timerInit(action.minutes);
    case "toggle": {
      if (state.running) return { ...state, running: false, endsAt: null, remainingMs: Math.max(0, (state.endsAt ?? action.now) - action.now) };
      const from = state.done || state.remainingMs <= 0 ? state.minutes * 60_000 : state.remainingMs;
      return { ...state, running: true, done: false, remainingMs: from, endsAt: action.now + from };
    }
    case "tick": {
      if (!state.running || state.endsAt === null) return state;
      if (action.now >= state.endsAt) return { ...state, running: false, endsAt: null, remainingMs: 0, done: true };
      const remainingMs = state.endsAt - action.now;
      return remainingMs === state.remainingMs ? state : { ...state, remainingMs };
    }
  }
}

/** `25:00`, or `1:05:00` from an hour up; a started second counts as a whole one. */
export function formatRemaining(ms: number): string {
  const total = Math.max(0, Math.ceil(ms / 1000));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const two = (n: number) => String(n).padStart(2, "0");
  return h > 0 ? `${h}:${two(m)}:${two(s)}` : `${two(m)}:${two(s)}`;
}
