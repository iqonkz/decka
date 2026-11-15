import { describe, expect, test } from "vitest";
import { PHOSPHOR_ICONS, searchIcons, svgFileName, svgUrl, visibleRows, WEIGHTS } from "../src/tabs/phosphor";

describe("the Phosphor index", () => {
  test("comes from the package: all icons, six weights", () => {
    expect(PHOSPHOR_ICONS.length).toBeGreaterThan(1500);
    expect(WEIGHTS).toEqual(["regular", "thin", "light", "bold", "fill", "duotone"]);
    expect(PHOSPHOR_ICONS.some((i) => i.name === "heart")).toBe(true);
  });

  test("file names and urls follow the package layout", () => {
    expect(svgFileName("heart", "regular")).toBe("heart.svg");
    expect(svgFileName("heart", "duotone")).toBe("heart-duotone.svg");
    expect(svgUrl("arrow-left", "bold")).toBe("phosphor/bold/arrow-left-bold.svg");
  });
});

describe("searchIcons", () => {
  const list = [
    { name: "heart", tags: ["love", "like", "favorite"] },
    { name: "heart-break", tags: ["sad"] },
    { name: "chat-heart", tags: ["message"] },
    { name: "star", tags: ["favorite", "rating"] },
  ];

  test("an empty query lists everything", () => {
    expect(searchIcons("", list)).toHaveLength(4);
  });

  test("matches names and tags; names starting with the word come first", () => {
    expect(searchIcons("heart", list).map((e) => e.name)).toEqual(["heart", "heart-break", "chat-heart"]);
    expect(searchIcons("favorite", list).map((e) => e.name)).toEqual(["heart", "star"]);
  });

  test("every word must match; case is ignored", () => {
    expect(searchIcons("HEART sad", list).map((e) => e.name)).toEqual(["heart-break"]);
    expect(searchIcons("nothing here", list)).toEqual([]);
  });

  test("the real index finds hearts by name and by tag", () => {
    expect(searchIcons("heart").length).toBeGreaterThan(5);
    expect(searchIcons("heart")[0]?.name).toMatch(/^heart/);
  });
});

describe("visibleRows (virtualised grid)", () => {
  test("only the rows in view plus an overscan are rendered", () => {
    expect(visibleRows(0, 280, 40, 200, 2)).toEqual([0, 9]);
    expect(visibleRows(400, 280, 40, 200, 2)).toEqual([8, 19]);
  });

  test("never outside the list; empty list renders nothing", () => {
    expect(visibleRows(100000, 280, 40, 50)).toEqual([41, 50]);
    expect(visibleRows(0, 280, 40, 0)).toEqual([0, 0]);
    expect(visibleRows(0, 280, 0, 10)).toEqual([0, 0]);
  });

  test("a grid of 1500 icons in 8 columns renders a few dozen cells, not all", () => {
    const rows = Math.ceil(1512 / 8);
    const [a, b] = visibleRows(0, 280, 44, rows);
    expect((b - a) * 8).toBeLessThan(100);
  });
});
