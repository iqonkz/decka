export const FAB_STORAGE_KEY = "decka.voicefab.v1";
export const FAB_MENU_KEY = "decka.voice.enabled.v1";
export const HOLD_TO_TALK_THRESHOLD_MS = 250;
export const DRAG_SLOP_PX = 8;
export const FAB_DEFAULT_SIZE = 56;
export const FAB_MARGIN = 16;

export interface Point {
  x: number;
  y: number;
}

export interface Viewport {
  width: number;
  height: number;
}

export type VoiceGesture = "talk" | "drag" | "pending";

/**
 * Snaps a floating button to the closest lateral edge (left or right)
 * and clamps within the screen bounds.
 */
export function snapToEdge(
  pos: Point,
  viewport: Viewport,
  size = FAB_DEFAULT_SIZE,
  margin = FAB_MARGIN,
): Point {
  const minX = margin;
  const maxX = Math.max(margin, viewport.width - size - margin);
  const minY = margin;
  const maxY = Math.max(margin, viewport.height - size - margin);

  const centerX = pos.x + size / 2;
  const targetX = centerX < viewport.width / 2 ? minX : maxX;
  const clampedY = Math.min(Math.max(pos.y, minY), maxY);

  return { x: targetX, y: clampedY };
}

/**
 * Distinguishes between dragging and holding to talk:
 * after 250 ms without movement beyond 8 px -> "talk";
 * movement beyond 8 px -> "drag";
 * otherwise -> "pending".
 */
export function detectVoiceGesture(
  dx: number,
  dy: number,
  elapsedMs: number,
  slop = DRAG_SLOP_PX,
  holdMs = HOLD_TO_TALK_THRESHOLD_MS,
): VoiceGesture {
  const dist = Math.hypot(dx, dy);
  if (dist > slop) {
    return "drag";
  }
  if (elapsedMs >= holdMs) {
    return "talk";
  }
  return "pending";
}

export function loadFabPosition(
  viewport: Viewport,
  size = FAB_DEFAULT_SIZE,
  margin = FAB_MARGIN,
): Point {
  try {
    const raw = localStorage.getItem(FAB_STORAGE_KEY);
    if (raw) {
      const parsed = JSON.parse(raw);
      if (typeof parsed.x === "number" && typeof parsed.y === "number") {
        return snapToEdge(parsed, viewport, size, margin);
      }
    }
  } catch {
    // Ignore storage errors
  }
  // Default position: bottom-right
  const defaultPos = {
    x: Math.max(margin, viewport.width - size - margin),
    y: Math.max(margin, viewport.height - size - margin - 20),
  };
  return snapToEdge(defaultPos, viewport, size, margin);
}

export function saveFabPosition(pos: Point): void {
  try {
    localStorage.setItem(FAB_STORAGE_KEY, JSON.stringify(pos));
  } catch {
    // Ignore storage errors
  }
}

export function loadVoiceButtonEnabled(): boolean {
  try {
    const val = localStorage.getItem(FAB_MENU_KEY);
    if (val === "false") return false;
    return true; // default enabled
  } catch {
    return true;
  }
}

export function saveVoiceButtonEnabled(enabled: boolean): void {
  try {
    localStorage.setItem(FAB_MENU_KEY, enabled ? "true" : "false");
  } catch {
    // Ignore storage errors
  }
}
