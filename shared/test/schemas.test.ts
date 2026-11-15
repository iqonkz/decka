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
  };
  return map[name] ?? null; // config.example has no schema
};

const valid = (ref: string, data: unknown): boolean => {
  const v = ajv.getSchema(ref);
  if (!v) throw new Error(`schema not found: ${ref}`);
  return v(data) as boolean;
};

describe("fixtures are valid", () => {
  test("23 validated fixtures", () => expect(fixtures.length).toBe(23));

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

  test("sanity: the minimal layout helper is valid", () => {
    expect(valid(layoutRef, layoutWith(button))).toBe(true);
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
  ];

  test("14 negative cases", () => expect(cases.length).toBe(14));

  test("grids from 3x2 to 8x5 are valid", () => {
    for (const grid of [{ cols: 3, rows: 2 }, { cols: 7, rows: 4 }, { cols: 8, rows: 5 }]) {
      expect(valid(layoutRef, layoutWith(button, grid)), JSON.stringify(grid)).toBe(true);
    }
  });
  test.each(cases)("%s", (_title, ref, data) => {
    expect(valid(ref, data)).toBe(false);
  });
});
