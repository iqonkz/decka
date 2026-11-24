import { describe, expect, it } from "vitest";
import { EDITOR_MAIN_MIN_PX, LIBRARY_COLLAPSE_PX, libraryCollapsed } from "./editorWidth";

describe("Keys page width (T23.B)", () => {
  it("collapses the library into a button below 1100 px of content", () => {
    expect(LIBRARY_COLLAPSE_PX).toBe(1100);
    expect(libraryCollapsed(1099)).toBe(true);
    expect(libraryCollapsed(1100)).toBe(false);
    expect(libraryCollapsed(1600)).toBe(false);
    expect(libraryCollapsed(640)).toBe(true);
  });

  it("does not collapse before the first measurement and keeps a minimum width for the preview", () => {
    expect(libraryCollapsed(0)).toBe(false);
    // Library 300 + gap 12 leaves at least the minimum of the preview at the threshold.
    expect(LIBRARY_COLLAPSE_PX - 300 - 12).toBeGreaterThanOrEqual(EDITOR_MAIN_MIN_PX);
  });
});
