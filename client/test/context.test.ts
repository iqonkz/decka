import { beforeEach, describe, expect, test } from "vitest";
import type { Page, WelcomeMsg } from "@decka/shared";
import welcomeFixture from "@decka/shared/fixtures/s2c-welcome.json";
import { decideContextPage, type ContextInput } from "../src/deck/context";
import { applyPatch, applyWelcome, getState, navigateManually, noteTouch, resetState, runContext, setState } from "../src/store/store";

const pages: Page[] = [
  { id: "main", name: "Main", items: [] },
  { id: "dev", name: "VS Code", apps: ["code.exe"], items: [] },
  { id: "web", name: "Browser", apps: ["chrome.exe", "msedge.exe"], items: [] },
];

const base: ContextInput = {
  pages,
  display: {},
  focusApp: { app: "code.exe" },
  pageIndex: 0,
  homePageIndex: 0,
  autoPageId: null,
  now: 100_000,
  lastTouch: 0,
};
const decide = (over: Partial<ContextInput> = {}) => decideContextPage({ ...base, ...over });

describe("decideContextPage (DATA_MODEL.md 6.1)", () => {
  test("1: contextPages === false changes nothing", () => {
    expect(decide({ display: { contextPages: false } })).toEqual({ pageIndex: 0, autoPageId: null, hint: null });
  });

  test("1: a missing contextPages means enabled", () => {
    expect(decide({ display: {} }).pageIndex).toBe(1);
  });

  test("2: a touch less than 5 s ago postpones, 5 s later it switches", () => {
    expect(decide({ now: 100_000, lastTouch: 97_000 }).pageIndex).toBe(0); // 3 s ago
    expect(decide({ now: 102_000, lastTouch: 97_000 }).pageIndex).toBe(1); // 5 s ago
  });

  test("3-4: the page whose apps contain the active app opens, with a hint", () => {
    expect(decide()).toEqual({ pageIndex: 1, autoPageId: "dev", hint: "VS Code" });
    expect(decide({ focusApp: { app: "msedge.exe" } })).toEqual({ pageIndex: 2, autoPageId: "web", hint: "Browser" });
  });

  test("4: already on the target page changes nothing", () => {
    expect(decide({ pageIndex: 1 })).toEqual({ pageIndex: 1, autoPageId: null, hint: null });
  });

  test("5: leaving the app returns home from the automatic page", () => {
    const d = decide({ focusApp: { app: "notepad.exe" }, pageIndex: 1, autoPageId: "dev", homePageIndex: 0 });
    expect(d).toEqual({ pageIndex: 0, autoPageId: null, hint: null });
  });

  test("5: home is the page the user chose by hand", () => {
    const d = decide({ focusApp: { app: null }, pageIndex: 1, autoPageId: "dev", homePageIndex: 2 });
    expect(d.pageIndex).toBe(2);
  });

  test("6: leaving the app does nothing when the user is on another page", () => {
    const d = decide({ focusApp: { app: "notepad.exe" }, pageIndex: 2, autoPageId: "dev" });
    expect(d).toEqual({ pageIndex: 2, autoPageId: "dev", hint: null });
  });

  test("an absent or null focus section is no application", () => {
    expect(decide({ focusApp: null })).toEqual({ pageIndex: 0, autoPageId: null, hint: null });
    expect(decide({ focusApp: undefined })).toEqual({ pageIndex: 0, autoPageId: null, hint: null });
  });

  test("an automatic page removed from the layout sends the user home", () => {
    const d = decide({ focusApp: { app: "notepad.exe" }, pages: [pages[0]!, pages[2]!], pageIndex: 1, autoPageId: "dev", homePageIndex: 0 });
    expect(d).toEqual({ pageIndex: 0, autoPageId: null, hint: null });
  });
});

describe("context pages in the store", () => {
  const welcome = structuredClone(welcomeFixture) as unknown as WelcomeMsg;
  welcome.state.layout.pages = pages;
  const focusPatch = (rev: number, app: string | null) => ({ t: "patch", rev, section: "focus", value: { app } }) as never;

  beforeEach(() => {
    resetState();
    applyWelcome(structuredClone(welcome));
  });

  test("a focus patch opens the page and leaving the app returns", () => {
    applyPatch(focusPatch(100, "code.exe"));
    expect(getState().pageIndex).toBe(1);
    expect(getState().autoPageId).toBe("dev");
    expect(getState().hint?.text).toBe("VS Code");
    applyPatch(focusPatch(101, "notepad.exe"));
    expect(getState().pageIndex).toBe(0);
    expect(getState().autoPageId).toBeNull();
  });

  test("a recent touch postpones, the timer tick applies it later", () => {
    noteTouch(Date.now());
    applyPatch(focusPatch(100, "code.exe"));
    expect(getState().pageIndex).toBe(0);
    runContext(Date.now() + 6000);
    expect(getState().pageIndex).toBe(1);
  });

  test("manual navigation on a context page cancels the automatic return", () => {
    applyPatch(focusPatch(100, "code.exe"));
    navigateManually(1); // the user swipes to / stays on the context page on purpose
    expect(getState().autoPageId).toBeNull();
    expect(getState().homePageIndex).toBe(1);
    applyPatch(focusPatch(101, "notepad.exe"));
    expect(getState().pageIndex).toBe(1);
  });

  test("welcome without a focus section changes nothing", () => {
    expect(getState().pageIndex).toBe(0);
    setState({ homePageIndex: 0 });
    runContext();
    expect(getState().pageIndex).toBe(0);
  });

  test("a layout patch that removes the automatic page returns home", () => {
    applyPatch(focusPatch(100, "code.exe"));
    const layout = { ...getState().state!.layout, pages: [pages[0]!, pages[2]!] };
    applyPatch({ t: "patch", rev: 101, section: "layout", value: layout } as never);
    expect(getState().pageIndex).toBe(0);
    expect(getState().autoPageId).toBeNull();
  });

  test("the hint is not set when nothing was switched", () => {
    applyPatch(focusPatch(100, "notepad.exe"));
    expect(getState().hint).toBeNull();
  });
});
