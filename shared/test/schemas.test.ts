import Ajv2020 from "ajv/dist/2020";
import { describe, expect, test } from "vitest";
import protocolSchema from "../schemas/protocol.schema.json";
import layoutSchema from "../schemas/layout.schema.json";
import actionsSchema from "../schemas/actions.schema.json";
import c2s_pair from "../fixtures/c2s-pair.json";
import s2c_paired from "../fixtures/s2c-paired.json";
import c2s_hello from "../fixtures/c2s-hello.json";
import s2c_welcome from "../fixtures/s2c-welcome.json";
import s2c_patch_vars from "../fixtures/s2c-patch-vars.json";
import c2s_press from "../fixtures/c2s-press.json";
import s2c_press_result_ok from "../fixtures/s2c-press-result-ok.json";
import s2c_press_result_err from "../fixtures/s2c-press-result-err.json";
import c2s_notif_dismiss from "../fixtures/c2s-notif-dismiss.json";
import c2s_ping from "../fixtures/c2s-ping.json";
import s2c_pong from "../fixtures/s2c-pong.json";
import s2c_error from "../fixtures/s2c-error.json";
import http_notify_req from "../fixtures/http-notify-req.json";
import http_status_req from "../fixtures/http-status-req.json";
import http_var_req from "../fixtures/http-var-req.json";
import http_error from "../fixtures/http-error.json";
import actions_default from "../fixtures/actions.default.json";
import layout_default from "../fixtures/layout.default.json";
import s2c_patch_focus from "../fixtures/s2c-patch-focus.json";
import s2c_patch_media from "../fixtures/s2c-patch-media.json";
import layout_context from "../fixtures/layout.context.json";
import s2c_patch_icons from "../fixtures/s2c-patch-icons.json";
import layout_popup from "../fixtures/layout.popup.json";
import actions_m15 from "../fixtures/actions.m15.json";
import state_m15 from "../fixtures/state.m15.json";

const fixtures: [string, unknown][] = [
  ["c2s-pair", c2s_pair],
  ["s2c-paired", s2c_paired],
  ["c2s-hello", c2s_hello],
  ["s2c-welcome", s2c_welcome],
  ["s2c-patch-vars", s2c_patch_vars],
  ["c2s-press", c2s_press],
  ["s2c-press-result-ok", s2c_press_result_ok],
  ["s2c-press-result-err", s2c_press_result_err],
  ["c2s-notif-dismiss", c2s_notif_dismiss],
  ["c2s-ping", c2s_ping],
  ["s2c-pong", s2c_pong],
  ["s2c-error", s2c_error],
  ["http-notify-req", http_notify_req],
  ["http-status-req", http_status_req],
  ["http-var-req", http_var_req],
  ["http-error", http_error],
  ["actions.default", actions_default],
  ["layout.default", layout_default],
  ["s2c-patch-focus", s2c_patch_focus],
  ["s2c-patch-media", s2c_patch_media],
  ["layout.context", layout_context],
  ["s2c-patch-icons", s2c_patch_icons],
  ["layout.popup", layout_popup],
  ["actions.m15", actions_m15],
  ["state.m15", state_m15],
];

const ajv = new Ajv2020({ allErrors: true, strict: false });
for (const s of [protocolSchema, layoutSchema, actionsSchema]) ajv.addSchema(s);
const ID = "https://decka.invalid/schemas";
const refFor = (name: string): string | null => {
  if (name.startsWith("c2s-")) return `${ID}/protocol.schema.json#/$defs/ClientMsg`;
  if (name.startsWith("s2c-")) return `${ID}/protocol.schema.json#/$defs/ServerMsg`;
  const map: Record<string, string> = {
    "http-notify-req": `${ID}/protocol.schema.json#/$defs/NotifyReq`,
    "http-status-req": `${ID}/protocol.schema.json#/$defs/StatusReq`,
    "http-var-req": `${ID}/protocol.schema.json#/$defs/VarReq`,
    "http-error": `${ID}/protocol.schema.json#/$defs/HttpError`,
    "actions.default": `${ID}/actions.schema.json#/$defs/ActionsFile`,
    "layout.default": `${ID}/layout.schema.json#/$defs/LayoutFile`,
    "layout.context": `${ID}/layout.schema.json#/$defs/LayoutFile`,
    "layout.popup": `${ID}/layout.schema.json#/$defs/LayoutFile`,
    "actions.m15": `${ID}/actions.schema.json#/$defs/ActionsFile`,
    "state.m15": `${ID}/protocol.schema.json#/$defs/FullState`,
  };
  return map[name] ?? null; // config.example has no schema
};

const valid = (ref: string, data: unknown): boolean => {
  const v = ajv.getSchema(ref);
  if (!v) throw new Error(`schema not found: ${ref}`);
  return v(data) as boolean;
};

describe("fixtures are valid", () => {
  test("25 validated fixtures", () => expect(fixtures.length).toBe(25));

  test.each(fixtures)("%s", (name, data) => {
    const ref = refFor(name) as string;
    const ok = valid(ref, data);
    if (!ok) console.error(ajv.getSchema(ref)?.errors);
    expect(ok).toBe(true);
  });
});
describe("invalid inputs are rejected", () => {
  const proto = (n: string) => `${ID}/protocol.schema.json#/$defs/${n}`;
  const layoutRef = `${ID}/layout.schema.json#/$defs/LayoutFile`;
  const actionsRef = `${ID}/actions.schema.json#/$defs/ActionsFile`;
  const client = { name: "Galaxy S9", app: "capacitor", version: "0.1.0" };

  const layoutWith = (item: Record<string, unknown>, grid = { cols: 5, rows: 3 }) => ({
    version: 1,
    grid,
    display: { dimAfterSec: 60, dimLevel: 0.3, sleepAfterSec: 600, pixelShift: true, night: { enabled: false, from: "23:00", to: "07:00" } },
    pages: [{ id: "main", name: "Main", items: [item] }],
  });
  const button = { id: "b", kind: "button", x: 0, y: 0, w: 1, h: 1, actionId: "lock" };

  test("T24.1: weather hourly is six hours with a local HH:mm, or absent", () => {
    const base = { place: "x", units: "metric", updatedAt: 1, current: { temp: 1, code: 0, isDay: true, wind: 1 }, daily: [0, 1, 2].map((d) => ({ date: `2026-11-1${6 + d}`, min: 0, max: 1, code: 0 })) };
    const hour = (h: number) => ({ time: `${String(h).padStart(2, "0")}:00`, temp: 3, code: 3, isDay: h > 6 && h < 18 });
    const weather = proto("WeatherInfo");
    expect(valid(weather, base)).toBe(true);
    expect(valid(weather, { ...base, hourly: [15, 16, 17, 18, 19, 20].map(hour) })).toBe(true);
    expect(valid(weather, { ...base, hourly: [15, 16, 17, 18, 19].map(hour) })).toBe(false);
    expect(valid(weather, { ...base, hourly: [15, 16, 17, 18, 19, 20].map((h) => ({ ...hour(h), time: "2026-11-16T15:00" })) })).toBe(false);
    expect(valid(weather, { ...base, hourly: [15, 16, 17, 18, 19, 20].map((h) => ({ ...hour(h), isDay: 1 })) })).toBe(false);
  });

  test("T24.4: a reminder of the phone section may carry a known repeat", () => {
    const section = proto("RemindersSection");
    const item = { id: "0123456789abcdef", text: "pills", at: "2026-11-26T09:00" };
    expect(valid(section, { items: [item] })).toBe(true);
    for (const repeat of ["daily", "weekdays", "weekly"]) expect(valid(section, { items: [{ ...item, repeat }] })).toBe(true);
    expect(valid(section, { items: [{ ...item, repeat: "monthly" }] })).toBe(false);
    expect(valid(section, { items: [{ ...item, repeat: null }] })).toBe(false);
  });

  test("T24.2: lyrics_offset is a key and a step of 500, and the lyrics section is bounded", () => {
    const msg = { t: "lyrics_offset", trackKey: "0123456789abcdef", deltaMs: 500 };
    expect(valid(proto("LyricsOffsetMsg"), msg)).toBe(true);
    expect(valid(proto("LyricsOffsetMsg"), { ...msg, deltaMs: -500 })).toBe(true);
    expect(valid(proto("ClientMsg"), msg)).toBe(true);
    for (const bad of [{ ...msg, deltaMs: 1000 }, { ...msg, deltaMs: 0 }, { ...msg, trackKey: "../x" }, { ...msg, trackKey: "ABCDEF0123456789" }, { ...msg, text: "la la" }, { t: "lyrics_offset", deltaMs: 500 }]) {
      expect(valid(proto("LyricsOffsetMsg"), bad)).toBe(false);
    }
    const info = { trackKey: "0123456789abcdef", synced: true, offsetMs: 1500, lines: [{ t: 1000, text: "la" }, { text: "" }] };
    expect(valid(proto("LyricsInfo"), info)).toBe(true);
    expect(valid(proto("LyricsInfo"), { ...info, offsetMs: 30500 })).toBe(false);
    expect(valid(proto("LyricsInfo"), { ...info, offsetMs: 250 })).toBe(false);
    expect(valid(proto("LyricsInfo"), { ...info, lines: Array.from({ length: 301 }, () => ({ text: "x" })) })).toBe(false);
    expect(valid(proto("LyricsInfo"), { ...info, lines: [{ text: "x".repeat(201) }] })).toBe(false);
  });

  test("list_toggle carries two ids issued by the agent and nothing else", () => {
    const ok = { t: "list_toggle", listId: "0123456789abcdef", itemId: "fedcba9876543210" };
    expect(valid(proto("ListToggleMsg"), ok)).toBe(true);
    expect(valid(proto("ClientMsg"), ok)).toBe(true);
    expect(valid(proto("ListToggleMsg"), { ...ok, text: "buy milk" })).toBe(false);
    expect(valid(proto("ListToggleMsg"), { ...ok, itemId: "../x" })).toBe(false);
    expect(valid(proto("ListToggleMsg"), { t: "list_toggle", listId: "0123456789abcdef" })).toBe(false);
  });

  test("M22: the reminders and lists sections and the two widgets (DATA_MODEL.md 13.7)", () => {
    const state = `${ID}/protocol.schema.json#/$defs/FullState`;
    const base = state_m15 as Record<string, unknown>;
    const id = "0123456789abcdef";
    const ok = { ...base, reminders: { items: [{ id, text: "Call mom", at: "2026-11-25T18:30" }] }, lists: { lists: [{ id, name: "Shopping", items: [{ id, text: "milk", done: false }] }] } };
    expect(valid(state, ok)).toBe(true);
    expect(valid(state, { ...base, reminders: { items: Array.from({ length: 6 }, () => ({ id, text: "x", at: "2026-11-25T18:30" })) } })).toBe(false); // at most 5
    expect(valid(state, { ...base, lists: { lists: [{ id, name: "x", items: Array.from({ length: 51 }, () => ({ id, text: "x", done: false })) }] } })).toBe(false); // at most 50
    expect(valid(state, { ...base, reminders: { items: [{ id, text: "x", at: "tomorrow" }] } })).toBe(false);
    expect(valid(layoutRef, layoutWith({ id: "w", kind: "reminders", x: 0, y: 0, w: 2, h: 2 }))).toBe(true);
    expect(valid(layoutRef, layoutWith({ id: "w", kind: "list", x: 0, y: 0, w: 2, h: 2, listId: id }))).toBe(true);
    expect(valid(layoutRef, layoutWith({ id: "w", kind: "list", x: 0, y: 0, w: 2, h: 2, listId: "../x" }))).toBe(false);
  });

  test("notif_open carries nothing but the id of a notification", () => {
    expect(valid(proto("NotifOpenMsg"), { t: "notif_open", notifId: "12" })).toBe(true);
    expect(valid(proto("NotifOpenMsg"), { t: "notif_open", notifId: "12", aumid: "x" })).toBe(false);
    expect(valid(proto("NotifOpenMsg"), { t: "notif_open", notifId: "../x" })).toBe(false);
    expect(valid(proto("ClientMsg"), { t: "notif_open", notifId: "3" })).toBe(true);
  });

  test("system ops: the M17 ones pass, an unknown one is rejected", () => {
    const file = (op: string) => ({ version: 1, actions: { x: { name: "x", type: "system", op } } });
    for (const op of ["lock", "sleep", "shutdown", "restart", "signout", "hibernate", "display_off", "task_manager", "settings", "explorer", "empty_recycle_bin"]) {
      expect(valid(actionsRef, file(op)), op).toBe(true);
    }
    expect(valid(actionsRef, file("format_c"))).toBe(false);
  });

  test("sanity: the minimal layout helper is valid", () => {
    expect(valid(layoutRef, layoutWith(button))).toBe(true);
  });

  test("the icons section takes PNG and SVG data URLs only", () => {
    const fs = `${ID}/protocol.schema.json#/$defs/FullState`;
    const base = (s2c_welcome as unknown as { state: Record<string, unknown> }).state;
    const state = (value: string) => ({ ...base, icons: { "0123456789abcdef": value } });
    expect(valid(fs, state("data:image/svg+xml;base64,PHN2Zz48L3N2Zz4="))).toBe(true);
    expect(valid(fs, state("data:image/png;base64,iVBORw0KGgo="))).toBe(true);
    expect(valid(fs, state("data:text/html;base64,PGI+"))).toBe(false);
  });

  test("theme, accent and wallpaper are checked (M13)", () => {
    const withDisplay = (extra: Record<string, unknown>) => {
      const l = layoutWith(button) as { display: Record<string, unknown> };
      Object.assign(l.display, extra);
      return l;
    };
    expect(valid(layoutRef, withDisplay({ theme: "glass", accent: "#7c5cff", wallpaper: "album" }))).toBe(true);
    expect(valid(layoutRef, withDisplay({ theme: "neon" }))).toBe(false);
    expect(valid(layoutRef, withDisplay({ accent: "purple" }))).toBe(false);
    expect(valid(layoutRef, withDisplay({ wallpaper: "forest" }))).toBe(false);
  });

  test("key face and labels (M14): known values only", () => {
    const withDisplay = (extra: Record<string, unknown>) => {
      const l = layoutWith(button) as { display: Record<string, unknown> };
      Object.assign(l.display, extra);
      return l;
    };
    expect(valid(layoutRef, withDisplay({ keyFace: "full", keyLabels: false }))).toBe(true);
    expect(valid(layoutRef, withDisplay({ keyFace: "huge" }))).toBe(false);
    expect(valid(layoutRef, withDisplay({ keyLabels: "yes" }))).toBe(false);
    for (const language of ["auto", "en", "zh", "hi", "es", "ar", "fr", "pt", "ru", "de", "ja", "kk"]) expect(valid(layoutRef, withDisplay({ language }))).toBe(true);
    expect(valid(layoutRef, withDisplay({ language: "xx" }))).toBe(false);
    expect(valid(layoutRef, withDisplay({ orientation: "portrait" }))).toBe(true);
    expect(valid(layoutRef, withDisplay({ orientation: "diagonal" }))).toBe(false);
    expect(valid(layoutRef, layoutWith({ ...button, face: "full", showLabel: false }))).toBe(true);
    expect(valid(layoutRef, layoutWith({ ...button, face: "big" }))).toBe(false);
    expect(valid(layoutRef, layoutWith({ ...button, showLabel: 1 }))).toBe(false);
  });

  test("img: icons need exactly 16 lower-case hex characters", () => {
    const withIcon = (icon: string) => layoutWith({ ...button, icon });
    expect(valid(layoutRef, withIcon("img:0123456789abcdef"))).toBe(true);
    expect(valid(layoutRef, withIcon("img:XYZ"))).toBe(false);
    expect(valid(layoutRef, withIcon("img:0123"))).toBe(false);
    expect(valid(layoutRef, withIcon("img:0123456789ABCDEF"))).toBe(false);
  });

  const cases: [string, string, unknown][] = [
    ["press without actionId", proto("ClientMsg"), { t: "press", id: 1 }],
    ["press with actionId 'Bad Id'", proto("ClientMsg"), { t: "press", id: 1, actionId: "Bad Id" }],
    ["hello with a 10-char token", proto("ClientMsg"), { t: "hello", v: 1, token: "abcdefghij", client }],
    ["NotifyReq with a 121-char title", proto("NotifyReq"), { title: "x".repeat(121) }],
    [
      "hotkey with key 'ctrl+c'",
      actionsRef,
      { version: 1, actions: { k: { name: "K", type: "hotkey", keys: ["ctrl+c"] } } },
    ],
    [
      "open_url with file:// url",
      actionsRef,
      { version: 1, actions: { u: { name: "U", type: "open_url", url: "file:///c:/x" } } },
    ],
    // "actionId + navigate" is no longer a schema matter: the code rule 12 refuses it (validate.test.ts).
    ["button with nothing to do (no actionId, navigate or popup)", layoutRef, layoutWith({ id: "b", kind: "button", x: 0, y: 0, w: 1, h: 1 })],
    ["button sized 2x1", layoutRef, layoutWith({ ...button, w: 2 })],
    ["patch value of section media is checked by the client, but MediaInfo needs a title", `${ID}/protocol.schema.json#/$defs/MediaInfo`, { source: "x", playing: true, positionAt: 1 }],
    ["MediaInfo with a non-image art", `${ID}/protocol.schema.json#/$defs/MediaInfo`, { source: "x", title: "t", playing: true, positionAt: 1, art: "http://x" }],
    ["FocusInfo with a window title instead of an exe", `${ID}/protocol.schema.json#/$defs/FocusInfo`, { app: "Untitled - Notepad" }],
    ["page with a non-exe app", layoutRef, { ...layoutWith(button), pages: [{ id: "main", name: "Main", items: [], apps: ["notepad"] }] }],
    ["grid 9x4", layoutRef, layoutWith(button, { cols: 9, rows: 4 })],
    ["grid 7x1", layoutRef, layoutWith(button, { cols: 7, rows: 1 })],
    // M15
    ["audio with an unknown op", actionsRef, { version: 1, actions: { a: { name: "A", type: "audio", op: "volume_up" } } }],
    ["obs with an unknown op", actionsRef, { version: 1, actions: { o: { name: "O", type: "obs", op: "restart" } } }],
    ["sequence with 11 steps", actionsRef, { version: 1, actions: { s: { name: "S", type: "sequence", steps: Array.from({ length: 11 }, () => ({ actionId: "lock" })) } } }],
    ["sequence with a 20 ms pause", actionsRef, { version: 1, actions: { s: { name: "S", type: "sequence", steps: [{ delayMs: 20 }] } } }],
    ["sequence step with both an action and a pause", actionsRef, { version: 1, actions: { s: { name: "S", type: "sequence", steps: [{ actionId: "lock", delayMs: 100 }] } } }],
    ["hotkey with a key outside the set (capslock)", actionsRef, { version: 1, actions: { k: { name: "K", type: "hotkey", keys: ["capslock"] } } }],
    ["volume sized 1x1", layoutRef, layoutWith({ id: "v", kind: "volume", x: 0, y: 0, w: 1, h: 1 })],
    ["timer sized 1x2", layoutRef, layoutWith({ id: "t", kind: "timer", x: 0, y: 0, w: 1, h: 2 })],
    ["weather sized 3x1", layoutRef, layoutWith({ id: "w", kind: "weather", x: 0, y: 0, w: 3, h: 1 })],
    ["obs sized 1x1", layoutRef, layoutWith({ id: "o", kind: "obs", x: 0, y: 0, w: 1, h: 1 })],
    ["timer with 5 presets", layoutRef, layoutWith({ id: "t", kind: "timer", x: 0, y: 0, w: 2, h: 1, presets: [1, 2, 3, 4, 5] })],
    ["sys without a graph sized 3x1", layoutRef, layoutWith({ id: "s", kind: "sys", x: 0, y: 0, w: 3, h: 1 })],
    ["WeatherInfo with 2 days", `${ID}/protocol.schema.json#/$defs/WeatherInfo`, { place: "x", units: "metric", updatedAt: 1, current: { temp: 1, code: 0, isDay: true, wind: 1 }, daily: [{ date: "2026-11-16", min: 0, max: 1, code: 0 }, { date: "2026-11-17", min: 0, max: 1, code: 0 }] }],
    ["ObsInfo without replayBuffer", `${ID}/protocol.schema.json#/$defs/ObsInfo`, { connected: true, recording: false, streaming: false }],
    ["MediaInfo with repeat 'all'", `${ID}/protocol.schema.json#/$defs/MediaInfo`, { source: "x", title: "t", playing: true, positionAt: 1, repeat: "all" }],
  ];

  test("negative cases", () => expect(cases.length).toBe(29));

  test("positive M15 shapes", () => {
    expect(valid(layoutRef, layoutWith({ id: "v", kind: "volume", x: 0, y: 0, w: 3, h: 1 }))).toBe(true);
    expect(valid(layoutRef, layoutWith({ id: "v", kind: "volume", x: 0, y: 0, w: 1, h: 4 }))).toBe(true);
    expect(valid(layoutRef, layoutWith({ id: "w", kind: "weather", x: 0, y: 0, w: 4, h: 2 }))).toBe(true);
    expect(valid(layoutRef, layoutWith({ id: "s", kind: "sys", x: 0, y: 0, w: 4, h: 2, graph: true }))).toBe(true);
    expect(valid(layoutRef, layoutWith({ ...button, actionId: undefined, navigate: "$back" }))).toBe(true);
    expect(valid(`${ID}/actions.schema.json#/$defs/ActionsFile`, { version: 1, actions: { k: { name: "K", type: "hotkey", keys: ["win", "period"] } } })).toBe(true);
  });

  test("grids from 3x2 to 8x5 are valid", () => {
    for (const grid of [{ cols: 3, rows: 2 }, { cols: 7, rows: 4 }, { cols: 8, rows: 5 }]) {
      expect(valid(layoutRef, layoutWith(button, grid)), JSON.stringify(grid)).toBe(true);
    }
  });
  test.each(cases)("%s", (_title, ref, data) => {
    expect(valid(ref, data)).toBe(false);
  });
});
