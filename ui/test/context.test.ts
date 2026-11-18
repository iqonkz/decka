import { describe, expect, test } from "vitest";
import { contextKeys, pageForApp } from "../src/widgets/Context";

const b = (id: string, x: number, y: number, extra: object = {}) => ({ id, kind: "button", x, y, w: 1, h: 1, ...extra });

describe("context widget helpers", () => {
  test("keys come in reading order and navigation keys are left out", () => {
    const items = [b("c", 0, 1), b("home", 6, 3, { navigate: "main" }), b("b", 1, 0), { id: "clock", kind: "clock", x: 3, y: 0, w: 2, h: 1 }, b("a", 0, 0)];
    expect(contextKeys(items as never).map((k) => k.id)).toEqual(["a", "b", "c"]);
  });

  test("the page of the app in front is found by its exe name", () => {
    const pages = [{ id: "one", apps: ["code.exe"] }, { id: "two", apps: ["olk.exe", "outlook.exe"] }];
    expect(pageForApp(pages, "outlook.exe")?.id).toBe("two");
    expect(pageForApp(pages, "notepad.exe")).toBeUndefined();
    expect(pageForApp(pages, null)).toBeUndefined();
    expect(pageForApp(undefined, "code.exe")).toBeUndefined();
  });
});
