import { beforeEach, describe, expect, test } from "vitest";
import type { Notification, NotifLevel, PatchMsg, WelcomeMsg } from "@decka/shared";
import welcomeFixture from "@decka/shared/fixtures/s2c-welcome.json";
import { relTime } from "../src/deck/widgets/relTime";
import { applyPatch, applyWelcome, getState, onAttention, resetState } from "../src/store/store";

const welcome = welcomeFixture as unknown as WelcomeMsg; // rev 42, one notification (id "7")
const n = (id: string, level: NotifLevel = "info"): Notification => ({ id, ts: 1, title: `t${id}`, level });
const notifPatch = (rev: number, list: Notification[]) =>
  ({ t: "patch", rev, section: "notifications", value: list }) as unknown as PatchMsg;

beforeEach(resetState);

describe("relTime", () => {
  const now = 1_000_000_000;
  test("now, minutes, hours, days", () => {
    expect(relTime(now - 5_000, now)).toBe("now");
    expect(relTime(now - 59_000, now)).toBe("now");
    expect(relTime(now - 60_000, now)).toBe("1m");
    expect(relTime(now - 59 * 60_000, now)).toBe("59m");
    expect(relTime(now - 60 * 60_000, now)).toBe("1h");
    expect(relTime(now - 23 * 3_600_000, now)).toBe("23h");
    expect(relTime(now - 24 * 3_600_000, now)).toBe("1d");
    expect(relTime(now - 3 * 86_400_000, now)).toBe("3d");
  });
});

describe("notification toasts", () => {
  test("a patch with one new id gives exactly one toast", () => {
    applyWelcome(welcome);
    applyPatch(notifPatch(43, [n("8", "warning"), ...(welcome.state.notifications as Notification[])]));
    const toasts = getState().toasts;
    expect(toasts).toHaveLength(1);
    expect(toasts[0]).toMatchObject({ level: "warning", title: "t8" });
    expect(toasts[0]!.until - Date.now()).toBeGreaterThan(7000); // 8 s
  });

  test("the same ids again give no new toast", () => {
    applyWelcome(welcome);
    const list = [n("8"), ...(welcome.state.notifications as Notification[])];
    applyPatch(notifPatch(43, list));
    applyPatch(notifPatch(44, list));
    expect(getState().toasts).toHaveLength(1);
  });

  test("notifications that arrive with the welcome do not toast", () => {
    const three = structuredClone(welcome);
    three.state.notifications = [n("3"), n("2"), n("1")];
    applyWelcome(three);
    expect(getState().toasts).toHaveLength(0);
  });

  test("onAttention fires once per new notification with its level", () => {
    const seen: NotifLevel[] = [];
    const off = onAttention((level) => seen.push(level));
    applyWelcome(welcome);
    applyPatch(notifPatch(43, [n("9", "error"), n("8", "info"), ...(welcome.state.notifications as Notification[])]));
    expect(seen.sort()).toEqual(["error", "info"]);
    off();
    applyPatch(notifPatch(44, [n("10", "error"), n("9", "error")]));
    expect(seen).toHaveLength(2);
  });

  test("removing a notification (dismiss) does not toast", () => {
    applyWelcome(welcome);
    applyPatch(notifPatch(43, []));
    expect(getState().toasts).toHaveLength(0);
  });
});
