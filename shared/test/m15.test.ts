import { describe, expect, test } from "vitest";
import { isReservedVar } from "../src/actions";
import { sizeAllowed, validateActions, validateLayout } from "../src/validate";
import type { Action, ActionsFile } from "../src/actions";
import type { ClientLayout, LayoutFile, LayoutItem } from "../src/layout";
import actionsM15 from "../fixtures/actions.m15.json";
import stateM15 from "../fixtures/state.m15.json";

const actions = actionsM15 as ActionsFile;
const layoutOf = (items: unknown[], extra: Partial<LayoutFile> = {}): LayoutFile => ({
  version: 1,
  grid: { cols: 7, rows: 4 },
  display: { dimAfterSec: 60, dimLevel: 0.3, sleepAfterSec: 600, pixelShift: true, night: { enabled: false, from: "23:00", to: "07:00" } },
  pages: [{ id: "main", name: "Main", items: items as LayoutItem[] }],
  ...extra,
});
const withAction = (id: string, action: Action): ActionsFile => ({ ...actions, actions: { ...actions.actions, [id]: action } });
const problems = (file: ActionsFile) => validateActions(file).map((i) => [i.path, i.message]);

describe("M15 fixtures", () => {
  test("actions.m15 has no issues", () => expect(validateActions(actions)).toEqual([]));

  test("the layout of state.m15 is valid against actions.m15", () => {
    const layout = { version: 1, ...(stateM15.layout as unknown as ClientLayout) } as LayoutFile;
    expect(validateLayout(layout, actions)).toEqual([]);
  });
});

describe("sequence (DATA_MODEL.md 2.3)", () => {
  const seq = (steps: unknown[]): Action => ({ name: "S", type: "sequence", steps: steps as never });

  test("a valid sequence has no issues", () => {
    expect(problems(withAction("s", seq([{ actionId: "media-play" }, { delayMs: 50 }, { actionId: "emoji-panel" }])))).toEqual([]);
  });

  test("11 steps are refused, 10 are fine", () => {
    const run = (n: number) => problems(withAction("s", seq(Array.from({ length: n }, () => ({ actionId: "media-play" })))));
    expect(run(10)).toEqual([]);
    expect(run(11)).toEqual([["/actions/s/steps", "a sequence has 1..10 steps"]]);
  });

  test("no steps at all is refused", () => {
    expect(problems(withAction("s", seq([])))).toEqual([["/actions/s/steps", "a sequence has 1..10 steps"]]);
  });

  test("a sequence inside a sequence is refused", () => {
    expect(problems(withAction("s", seq([{ actionId: "go-live" }])))).toEqual([["/actions/s/steps/0", "a sequence cannot contain another sequence"]]);
  });

  test("a sequence that runs itself is refused", () => {
    expect(problems(withAction("s", seq([{ actionId: "s" }])))).toEqual([["/actions/s/steps/0", "a sequence cannot contain another sequence"]]);
  });

  test("the pauses may add up to 15000 ms and not more", () => {
    const pauses = (list: number[]) => problems(withAction("s", seq(list.map((delayMs) => ({ delayMs })))));
    expect(pauses([10000, 5000])).toEqual([]);
    expect(pauses([10000, 5001])).toEqual([["/actions/s/steps", "the pauses add up to more than 15000 ms"]]);
  });

  test("a pause is 50..10000 ms, whole", () => {
    expect(problems(withAction("s", seq([{ delayMs: 49 }, { delayMs: 10001 }, { delayMs: 100.5 }])))).toHaveLength(3);
  });

  test("an action that does not exist is refused", () => {
    expect(problems(withAction("s", seq([{ actionId: "nope" }])))).toEqual([["/actions/s/steps/0", 'action "nope" does not exist']]);
  });
});

describe("obs actions", () => {
  const obs = (extra: Record<string, unknown>): Action => ({ name: "O", type: "obs", ...extra }) as Action;

  test("`scene` goes with op scene and `input` with input_mute_toggle", () => {
    expect(problems(withAction("o", obs({ op: "scene", scene: "Gaming" })))).toEqual([]);
    expect(problems(withAction("o", obs({ op: "input_mute_toggle", input: "Mic/Aux" })))).toEqual([]);
    expect(problems(withAction("o", obs({ op: "record_toggle" })))).toEqual([]);
  });

  test("op scene without a scene, and a scene without op scene, are refused", () => {
    expect(problems(withAction("o", obs({ op: "scene" }))).map(([path]) => path)).toEqual(["/actions/o/scene"]);
    expect(problems(withAction("o", obs({ op: "record_toggle", scene: "Gaming" }))).map(([path]) => path)).toEqual(["/actions/o/scene"]);
  });

  test("input_mute_toggle without an input, and an input with another op, are refused", () => {
    expect(problems(withAction("o", obs({ op: "input_mute_toggle" }))).map(([path]) => path)).toEqual(["/actions/o/input"]);
    expect(problems(withAction("o", obs({ op: "scene", scene: "A", input: "B" }))).map(([path]) => path)).toEqual(["/actions/o/input"]);
  });

  test("an empty or 101-character scene is refused", () => {
    expect(problems(withAction("o", obs({ op: "scene", scene: "" }))).map(([path]) => path)).toEqual(["/actions/o/scene"]);
    expect(problems(withAction("o", obs({ op: "scene", scene: "x".repeat(101) }))).map(([path]) => path)).toEqual(["/actions/o/scene"]);
  });
});

describe("reserved variables", () => {
  test("the prefixes of the collectors are reserved", () => {
    for (const name of ["audio.muted", "mic.muted", "media.playing", "obs.recording"]) expect(isReservedVar(name), name).toBe(true);
    for (const name of ["audio", "audiobook.x", "my.flag", "obs"]) expect(isReservedVar(name), name).toBe(false);
  });

  test("toggleVar on a reserved name is refused with the text of PROTOCOL.md", () => {
    const a: Action = { name: "Mute", type: "media", op: "mute", toggleVar: "audio.muted" };
    expect(problems(withAction("m", a))).toEqual([["/actions/m/toggleVar", 'toggleVar: "audio.muted" is set by Decka itself — remove it']]);
    expect(problems(withAction("m", { ...a, toggleVar: "my.flag" }))).toEqual([]);
  });
});

describe("widgets of M15 (DATA_MODEL.md 3.2 rule 13)", () => {
  const widget = (kind: string, w: number, h: number, extra: Record<string, unknown> = {}) => ({ id: "w", kind, x: 0, y: 0, w, h, ...extra });
  const issues = (item: unknown, extra?: Partial<LayoutFile>) => validateLayout(layoutOf([item], extra), actions).map((i) => i.message);

  test.each([
    ["volume", 1, 2, true], ["volume", 1, 4, true], ["volume", 2, 1, true], ["volume", 4, 1, true],
    ["volume", 1, 1, false], ["volume", 2, 2, false], ["volume", 5, 1, false], ["volume", 1, 5, false],
    ["timer", 1, 1, true], ["timer", 2, 1, true], ["timer", 2, 2, true], ["timer", 1, 2, false], ["timer", 3, 1, false],
    ["weather", 1, 1, true], ["weather", 2, 1, true], ["weather", 3, 2, true], ["weather", 4, 2, true],
    ["weather", 3, 1, false], ["weather", 2, 2, false], ["weather", 5, 2, false],
    ["obs", 2, 1, true], ["obs", 4, 2, true], ["obs", 1, 1, false], ["obs", 5, 1, false], ["obs", 2, 3, false],
  ] as const)("%s %ix%i is %s", (kind, w, h, allowed) => {
    expect(sizeAllowed(kind, w, h, { cols: 7, rows: 4 })).toBe(allowed);
    expect(issues(widget(kind, w, h)).length === 0).toBe(allowed);
  });

  test("sys with graph takes 2x1..4x2, without it only 2 wide", () => {
    expect(issues(widget("sys", 4, 2, { graph: true }))).toEqual([]);
    expect(issues(widget("sys", 2, 1, { graph: true }))).toEqual([]);
    expect(issues(widget("sys", 5, 2, { graph: true }))).toHaveLength(1);
    expect(issues(widget("sys", 3, 1))).toHaveLength(1);
  });

  test("the actions of a volume slider must exist", () => {
    expect(issues(widget("volume", 3, 1, { upAction: "vol-up", downAction: "vol-down", muteAction: "vol-mute" }))).toEqual([]);
    expect(issues(widget("volume", 3, 1, { upAction: "nope" }))).toEqual(["unknown upAction 'nope'"]);
    expect(issues(widget("volume", 3, 1, { muteAction: "nope" }))).toEqual(["unknown muteAction 'nope'"]);
  });

  test("timer presets: 1..4 whole numbers of 1..180", () => {
    expect(issues(widget("timer", 2, 1, { presets: [25, 5] }))).toEqual([]);
    expect(issues(widget("timer", 2, 1, { presets: [1, 2, 3, 4] }))).toEqual([]);
    for (const presets of [[], [1, 2, 3, 4, 5], [0], [181], [1.5]]) expect(issues(widget("timer", 2, 1, { presets })), JSON.stringify(presets)).toHaveLength(1);
  });
});

describe("navigate: $back (DATA_MODEL.md 3.8)", () => {
  const key = { id: "b", kind: "button", x: 0, y: 0, w: 1, h: 1, navigate: "$back" };

  test("a key of a normal page may go back", () => {
    expect(validateLayout(layoutOf([key]), actions)).toEqual([]);
  });

  test("a key inside a popup group may not", () => {
    const layout: LayoutFile = layoutOf([{ id: "o", kind: "button", x: 0, y: 0, w: 1, h: 1, popup: "grp" }]);
    layout.pages.push({ id: "grp", name: "Group", popup: true, items: [key as unknown as LayoutItem] });
    expect(validateLayout(layout, actions).map((i) => i.message)).toEqual(["a key inside a popup group cannot open a page or another group"]);
  });

  test("$back together with an action is still refused", () => {
    expect(validateLayout(layoutOf([{ ...key, actionId: "media-play" }]), actions).map((i) => i.message)).toEqual(["a button can either run an action or open a page, not both"]);
  });
});
