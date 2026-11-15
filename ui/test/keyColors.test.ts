import { describe, expect, test } from "vitest";
import { colorVars, parseHex } from "../src/theme/colors";
import { applyTheme, DEFAULT_ACCENT, resolveTheme } from "../src/theme/theme";

describe("colorVars", () => {
  test("no colour or garbage: no variables", () => {
    expect(colorVars(undefined)).toEqual({});
    expect(colorVars("red")).toEqual({});
  });

  test("a colour: 12% lighter on top, 18% darker at the bottom, tints and a readable ink", () => {
    // #2563eb = 37,99,235: +12% towards white, -18% towards black
    expect(colorVars("#2563eb")).toEqual({
      "--c": "#2563eb",
      "--c-on": "#ffffff",
      "--c-on-shadow": "0 1px 2px rgba(0,0,0,0.9)",
      // 12% lighter / 18% darker would be #3f76ed / #1e51c1; that is a mid tone where no label reaches 4.5:1,
      // so the face is darkened until a white label does.
      "--c-top": "#3b6fdf",
      "--c-bottom": "#1c4cb5",
      "--c-a16": "rgba(37,99,235,0.16)",
      "--c-a30": "rgba(37,99,235,0.30)",
      "--c-a32": "rgba(37,99,235,0.32)",
      "--c-a50": "rgba(37,99,235,0.5)",
      "--c-ink": "#2563eb",
    });
  });

  test("a colour darker than luminance 64 keeps a light ink; short hex works", () => {
    expect(colorVars("#202020")["--c-ink"]).toBe("#e5e5e5");
    // A white key keeps the white label (DESIGN.md 1.6), so its face is darkened until 4.5:1 instead.
    expect(colorVars("#fff")["--c-on"]).toBe("#ffffff");
    expect(colorVars("#fff")["--c-bottom"]).toBe("#606060");
    expect(parseHex("#abc")).toEqual([170, 187, 204]);
  });
});

describe("resolveTheme", () => {
  test("a missing theme is amoled, the accent and wallpaper have defaults", () => {
    expect(resolveTheme(undefined)).toEqual({ theme: "amoled", accent: DEFAULT_ACCENT, wallpaper: "aurora" });
    expect(resolveTheme({})).toEqual({ theme: "amoled", accent: "#7c5cff", wallpaper: "aurora" });
  });

  test("known values are kept, unknown ones fall back", () => {
    expect(resolveTheme({ theme: "glass", accent: "#16a34a", wallpaper: "album" })).toEqual({ theme: "glass", accent: "#16a34a", wallpaper: "album" });
    expect(resolveTheme({ theme: "neon" as never, accent: "green", wallpaper: "forest" as never })).toEqual({ theme: "amoled", accent: "#7c5cff", wallpaper: "aurora" });
  });

  test("applyTheme writes data attributes and the accent variable", () => {
    const attrs = new Map<string, string>();
    const root = { dataset: {} as Record<string, string>, style: { setProperty: (k: string, v: string) => attrs.set(k, v) } } as unknown as HTMLElement;
    applyTheme(root, { theme: "studio", accent: "#3b82f6" });
    expect(root.dataset.theme).toBe("studio");
    expect(root.dataset.wallpaper).toBe("aurora");
    expect(attrs.get("--accent")).toBe("#3b82f6");
  });
});
