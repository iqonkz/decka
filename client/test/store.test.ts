import { beforeEach, describe, expect, test } from "vitest";
import type { PatchMsg, WelcomeMsg } from "@decka/shared";
import welcomeFixture from "@decka/shared/fixtures/s2c-welcome.json";
import { applyPatch, applyWelcome, expireToasts, getState, pushToast, resetState, setState } from "../src/store/store";

const welcome = welcomeFixture as unknown as WelcomeMsg; // rev 42, one page
const patch = (rev: number, section: string, value: unknown) => ({ t: "patch", rev, section, value }) as unknown as PatchMsg;

beforeEach(resetState);

describe("applyWelcome", () => {
  test("sets state, rev, online and agent name", () => {
    applyWelcome(welcome);
    const s = getState();
    expect(s.conn).toBe("online");
    expect(s.rev).toBe(42);
    expect(s.agentName).toBe("DESKTOP-MAIN");
  });

  test("resets pageIndex when there are fewer pages now", () => {
    setState({ pageIndex: 3 });
    applyWelcome(welcome);
    expect(getState().pageIndex).toBe(0);
  });

  test("keeps a valid pageIndex", () => {
    const two = structuredClone(welcome);
    two.state.layout.pages.push({ id: "dev", name: "Dev", items: [] });
    setState({ pageIndex: 1 });
    applyWelcome(two);
    expect(getState().pageIndex).toBe(1);
  });
});

describe("applyPatch", () => {
  test("replaces the section and bumps rev", () => {
    applyWelcome(welcome);
    applyPatch(patch(43, "vars", { "mic.muted": true }));
    expect(getState().state?.vars).toEqual({ "mic.muted": true });
    expect(getState().rev).toBe(43);
  });

  test("an old rev changes nothing", () => {
    applyWelcome(welcome);
    const before = getState();
    applyPatch(patch(42, "vars", { x: 1 }));
    applyPatch(patch(10, "vars", { x: 1 }));
    expect(getState()).toBe(before);
  });

  test("an unknown section does not throw and changes nothing", () => {
    applyWelcome(welcome);
    const before = getState();
    expect(() => applyPatch(patch(50, "foo", { a: 1 }))).not.toThrow();
    expect(getState()).toBe(before);
  });

  test("a layout patch with fewer pages corrects pageIndex", () => {
    const two = structuredClone(welcome);
    two.state.layout.pages.push({ id: "dev", name: "Dev", items: [] });
    applyWelcome(two);
    setState({ pageIndex: 1 });
    applyPatch(patch(43, "layout", welcome.state.layout));
    expect(getState().pageIndex).toBe(0);
  });

  test("is ignored before the first welcome", () => {
    applyPatch(patch(1, "vars", {}));
    expect(getState().state).toBeNull();
  });
});

describe("toasts", () => {
  test("keep at most three and expire by time", () => {
    for (let i = 0; i < 4; i++) pushToast({ level: "info", title: `t${i}`, ms: 1000 * (i + 1) }, 0);
    expect(getState().toasts.map((t) => t.title)).toEqual(["t1", "t2", "t3"]);
    expireToasts(3500);
    expect(getState().toasts.map((t) => t.title)).toEqual(["t3"]);
  });
});
