import { renderToString } from "react-dom/server";
import { describe, expect, test } from "vitest";
import { DeckEnvContext, EMPTY_DATA, type DeckEnv } from "../src/deckEnv";
import Icon from "../src/Icon";
import { ICONS } from "../src/icons";

const NOOP = () => {};
const envWith = (icons: Record<string, string>): DeckEnv => ({ ...EMPTY_DATA, icons, onPress: () => Promise.resolve(), onPressError: NOOP, onNavigate: NOOP, onDismiss: NOOP });

// DATA_MODEL.md 3.6
const NAMES =
  "play, pause, skip-back, skip-forward, volume, volume-1, volume-2, volume-x, mic, mic-off, video, video-off, monitor, lock, power, moon, sun, home, settings, terminal, code, link, git-branch, globe, folder, file-text, activity, cpu, hard-drive, wifi, bell, bell-off, mail, message-square, music, headphones, camera, image, calendar, clock, timer, star, heart, zap, rocket, bug, check, x, plus, minus, refresh-cw, search, download, upload, copy, trash-2, square, circle, arrow-left, arrow-right".split(
    ", ",
  );

const render = (icon: string, icons: Record<string, string> = {}) => renderToString(<DeckEnvContext.Provider value={envWith(icons)}><Icon icon={icon} size={40} /></DeckEnvContext.Provider>);

describe("Icon", () => {
  test("the set has exactly 60 names", () => {
    expect(NAMES).toHaveLength(60);
    expect(Object.keys(ICONS).sort()).toEqual([...NAMES].sort());
  });

  test("every name renders an svg of the requested pixel size", () => {
    for (const name of NAMES) {
      const html = render(`lucide:${name}`);
      expect(html, name).toContain("<svg");
      expect(html, name).toContain('width="40"');
      expect(html, name).toContain('height="40"');
      expect(html, name).toContain('stroke="currentColor"');
      expect(html, name).toMatch(/<(path|line|circle|rect|polyline|polygon|ellipse)\b/); // the glyph itself, not an empty svg
    }
  });

  test("only unknown names fall back to the square", () => {
    const square = render("lucide:square");
    for (const name of NAMES.filter((n) => n !== "square")) expect(render(`lucide:${name}`), name).not.toBe(square);
    expect(render("lucide:no-such-icon")).toBe(square);
  });

  test("img: shows the picture from the icons section, a square without an entry", () => {
    const hash = "0123456789abcdef";
    const url = "data:image/png;base64,iVBORw0KGgo=";
    const html = render(`img:${hash}`, { [hash]: url });
    expect(html).toContain("<img");
    expect(html).toContain(url);
    expect(html).toContain('width="40"');
    expect(render("img:fedcba9876543210", { [hash]: url })).toBe(render("lucide:square"));
    expect(render("img:XYZ", { [hash]: url })).toBe("");
    expect(render(`img:${hash}`)).toBe(render("lucide:square")); // empty section
  });

  test("emoji renders a span, anything else nothing", () => {
    expect(render("emoji:🚀")).toContain("<span");
    expect(render("play")).toBe("");
    expect(render("")).toBe("");
  });
});
