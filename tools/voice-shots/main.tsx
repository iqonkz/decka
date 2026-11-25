// Harness of tools/voice-shots.mjs: draws the real PC pill (agent) and the real phone screens, widgets, and voice controls (client).
import { createRoot } from "react-dom/client";
import { setLanguage, type Language } from "@decka/shared";
import { recordSys, resetSysHistory } from "@decka/ui";
import VoiceFab from "../../client/src/voice/VoiceFab";
import VoiceConfirmSheet from "../../client/src/voice/VoiceConfirmSheet";
import { setState } from "../../client/src/store/store";

const SAMPLE_ART = `data:image/svg+xml;utf8,${encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" width="128" height="128"><defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0%" stop-color="#e11d48"/><stop offset="100%" stop-color="#f59e0b"/></linearGradient></defs><rect width="128" height="128" fill="url(#g)"/><circle cx="64" cy="64" r="34" fill="#fff" fill-opacity="0.25"/></svg>')}`;

// The demo layout refers to a few actions that the default set does not have: simple stand-ins, so the editor shows no problems.
function demoActions(base: { version: number; actions: Record<string, unknown> }) {
  const stub = (name: string, op: string) => ({ name, type: "media", op });
  return { ...base, actions: { ...base.actions, "mic-toggle": stub("Mic on", "mute"), shuffle: stub("Shuffle", "next"), "out-mute": stub("Mute", "mute"), "obs-record": stub("Record", "play_pause"), "go-live": stub("Go live", "play_pause") } };
}

const q = new URLSearchParams(location.search);
const scene = q.get("scene") ?? "pill";
const state = (q.get("state") ?? "listening") as "idle" | "listening" | "loading" | "recognizing" | "thinking" | "confirm" | "done" | "error";
if (q.get("lang")) Object.defineProperty(navigator, "language", { value: q.get("lang"), configurable: true });
setLanguage((q.get("lang") ?? "en") as Language);

const sample = {
  heard: q.get("heard") ?? "turn the volume up",
  reply: q.get("reply") ?? "Done",
  confirm: { text: "Lock the computer", key: "voice.confirm.lock" },
};
const info = {
  state,
  id: "0011223344556677",
  source: scene === "pill" ? "pc" : "phone",
  ...(state === "listening" ? {} : { heard: sample.heard }),
  ...(state === "done" || state === "error" ? { reply: sample.reply } : {}),
  ...(state === "confirm" ? { confirm: sample.confirm } : {}),
  ...(q.get("activity") ? { activity: q.get("activity") } : {}),
};

// M22 demo data (nothing personal): the next reminders and two lists.
const M22_LISTS = [
  { id: "a1b2c3d4e5f60718", name: "Shopping", items: [{ id: "1000000000000001", text: "Milk", done: true }, { id: "1000000000000002", text: "Bread", done: false }, { id: "1000000000000003", text: "Eggs", done: false }, { id: "1000000000000004", text: "Coffee beans", done: false }] },
  { id: "0f1e2d3c4b5a6978", name: "Trip", items: [{ id: "2000000000000001", text: "Passport", done: false }, { id: "2000000000000002", text: "Charger", done: true }] },
];
const M22_REMINDERS = [
  { id: "3000000000000001", text: "Call the dentist", at: "2026-11-25T16:30" },
  { id: "3000000000000002", text: "Morning standup", at: "2026-11-26T09:00", repeat: "weekdays" },
  { id: "3000000000000003", text: "Take vitamins", at: "2026-11-26T08:00", repeat: "daily" },
  { id: "3000000000000004", text: "Team weekly sync", at: "2026-11-30T11:00", repeat: "weekly" },
];

const root = createRoot(document.getElementById("root")!);
if (scene === "deck" || scene === "settings") {
  const [stateFx, actionsFx] = await Promise.all([import("../../shared/fixtures/state.m15.json"), import("../../shared/fixtures/actions.default.json")]);
  const fx = JSON.parse(JSON.stringify(stateFx.default)) as { layout: { display: Record<string, unknown>; pages: { items: unknown[] }[] }; [k: string]: unknown };
  fx.layout.display = { ...fx.layout.display, theme: q.get("theme") ?? "glass", wallpaper: q.get("wallpaper") ?? "aurora", orientation: "landscape" };
  if (q.get("lang")) fx.layout.display = { ...fx.layout.display, language: q.get("lang") };
  const pack = q.get("pack");
  if (q.get("m22")) {
    fx.layout.pages[0]!.items = [
      { id: "w-rem", kind: "reminders", x: 0, y: 0, w: 3, h: 2 },
      { id: "w-l1", kind: "list", x: 3, y: 0, w: 2, h: 2, listId: M22_LISTS[0]!.id },
      { id: "w-l2", kind: "list", x: 5, y: 0, w: 2, h: 2, listId: M22_LISTS[1]!.id },
      { id: "w-l3", kind: "list", x: 0, y: 2, w: 2, h: 1, listId: "ffffffffffffffff" },
      { id: "w-rem2", kind: "reminders", x: 2, y: 2, w: 3, h: 1 },
    ];
    fx.reminders = { items: M22_REMINDERS };
    fx.lists = { lists: M22_LISTS };
    fx.layout.pages[0]!.items.push({ id: "w-media", kind: "media", x: 2, y: 3, w: 3, h: 1, prevAction: "media-prev", playAction: "media-play", nextAction: "media-next" });
  } else if (q.get("orient") === "portrait") {
    fx.layout.display = { ...fx.layout.display, orientation: "portrait" };
    (fx.layout as Record<string, unknown>).grid = { cols: 4, rows: 7 };
    fx.layout.pages[0]!.items = [
      { id: "p-w", kind: "weather", x: 0, y: 0, w: 2, h: 2 },
      { id: "p-c", kind: "clock", x: 2, y: 0, w: 2, h: 2 },
      { id: "p-m", kind: "media", x: 0, y: 2, w: 4, h: 1, prevAction: "media-prev", playAction: "media-play", nextAction: "media-next" },
      { id: "p-v", kind: "volume", x: 0, y: 3, w: 4, h: 1, upAction: "vol-up", downAction: "vol-down", muteAction: "vol-mute" },
      { id: "p-b1", kind: "button", x: 0, y: 4, w: 1, h: 1, label: "Mic", icon: "lucide:mic", actionId: "mic-toggle" },
      { id: "p-b2", kind: "button", x: 1, y: 4, w: 1, h: 1, label: "Mute", icon: "lucide:volume-x", actionId: "out-mute", color: "#b45309" },
      { id: "p-b3", kind: "button", x: 2, y: 4, w: 1, h: 1, label: "REC", icon: "lucide:video", actionId: "obs-record", color: "#dc2626" },
      { id: "p-b4", kind: "button", x: 3, y: 4, w: 1, h: 1, label: "Live", icon: "lucide:rocket", actionId: "go-live" },
      { id: "p-s", kind: "sys", x: 0, y: 5, w: 4, h: 2, graph: true },
    ];
    (fx.media as Record<string, unknown>).art = SAMPLE_ART;
    (fx.media as Record<string, unknown>).album = "Hurry Up, We're Dreaming";
  } else if (pack === "sml1") {
    (fx.layout as Record<string, unknown>).grid = { cols: 8, rows: 6 };
    fx.layout.pages[0]!.items = [
      { id: "w-s", kind: "weather", x: 0, y: 0, w: 1, h: 1 },
      { id: "w-m", kind: "weather", x: 1, y: 0, w: 2, h: 2 },
      { id: "w-l", kind: "weather", x: 3, y: 0, w: 3, h: 2 },
      { id: "c-s", kind: "clock", x: 0, y: 1, w: 1, h: 1 },
      { id: "c-m", kind: "clock", x: 6, y: 0, w: 2, h: 2 },
      { id: "t-s", kind: "timer", x: 0, y: 2, w: 1, h: 1, presets: [25, 5] },
      { id: "v-s", kind: "volume", x: 1, y: 2, w: 2, h: 1, upAction: "vol-up", downAction: "vol-down", muteAction: "vol-mute" },
      { id: "t-m", kind: "timer", x: 3, y: 2, w: 2, h: 2, presets: [25, 10, 5] },
      { id: "v-m", kind: "volume", x: 5, y: 2, w: 3, h: 2, upAction: "vol-up", downAction: "vol-down", muteAction: "vol-mute" },
      { id: "c-l", kind: "clock", x: 0, y: 3, w: 3, h: 2 },
      { id: "t-l", kind: "timer", x: 5, y: 4, w: 3, h: 2, presets: [25, 15, 5] },
      { id: "v-l", kind: "volume", x: 0, y: 5, w: 5, h: 1, upAction: "vol-up", downAction: "vol-down", muteAction: "vol-mute" },
    ];
  } else if (pack === "sml2") {
    (fx.layout as Record<string, unknown>).grid = { cols: 8, rows: 5 };
    (fx.media as Record<string, unknown>).art = SAMPLE_ART;
    (fx.media as Record<string, unknown>).album = "Hurry Up, We're Dreaming";
    const nowMs = Date.now();
    fx.ai = {
      claude: { windows: [{ label: "5h", pct: 48, resetsAt: nowMs + 7200_000 }, { label: "7d", pct: 84, resetsAt: nowMs + 86400_000 * 2 }], updatedAt: nowMs },
      codex: { windows: [{ label: "30d", pct: 93, resetsAt: nowMs + 86400_000 * 5 }], plan: "Plus", updatedAt: nowMs },
      sessions: [{ id: "s1", project: "decka", model: "Sonnet", ctxPct: 34, lastSeen: nowMs }],
      pending: [],
      history: [],
    };
    fx.layout.pages[0]!.items = [
      { id: "m-s", kind: "media", x: 0, y: 0, w: 2, h: 1, prevAction: "media-prev", playAction: "media-play", nextAction: "media-next" },
      { id: "m-m", kind: "media", x: 2, y: 0, w: 3, h: 1, prevAction: "media-prev", playAction: "media-play", nextAction: "media-next" },
      { id: "m-l", kind: "media", x: 0, y: 1, w: 4, h: 2, prevAction: "media-prev", playAction: "media-play", nextAction: "media-next" },
      { id: "s-s", kind: "sys", x: 5, y: 0, w: 3, h: 1 },
      { id: "s-l", kind: "sys", x: 4, y: 1, w: 4, h: 2 },
      { id: "ai-s", kind: "ai", x: 0, y: 3, w: 2, h: 1, show: "both" },
      { id: "ai-m", kind: "ai", x: 2, y: 3, w: 2, h: 2, show: "both" },
      { id: "ai-l", kind: "ai", x: 4, y: 3, w: 4, h: 2, show: "both" },
    ];
  } else if (pack === "sml3") {
    (fx.layout as Record<string, unknown>).grid = { cols: 8, rows: 5 };
    const nowMs = Date.now();
    fx.twitch = { connected: true, login: "decka", live: true, viewers: 128, followers: 4200, title: "Coding Decka UI", game: "Software", startedAt: nowMs - 3600_000 };
    fx.youtube = { connected: true, channel: "Decka", live: false, viewers: 0, subscribers: 8900 };
    fx.battery = { devices: [{ kind: "pc", name: "PC", pct: 86 }, { kind: "bt", name: "Headset", pct: 18 }, { kind: "bt", name: "Mouse", pct: 7 }], updatedAt: nowMs };
    fx.mail = { accounts: [{ id: "m1", name: "Work", unread: 4 }, { id: "m2", name: "Personal", unread: 0 }], updatedAt: nowMs };
    fx.reminders = { items: M22_REMINDERS };
    fx.lists = { lists: M22_LISTS };
    fx.statuses = [{ id: "st1", label: "Build", state: "ok", text: "passed", source: "push", updatedAt: nowMs }, { id: "st2", label: "Deploy", state: "running", text: "1m", source: "check", updatedAt: nowMs }];
    fx.notifications = [{ id: "1", ts: nowMs - 120_000, title: "GitHub", body: "PR merged into main", level: "success", source: "github" }];
    fx.layout.pages[0]!.items = [
      { id: "o-l", kind: "obs", x: 0, y: 0, w: 2, h: 2 },
      { id: "tw-l", kind: "twitch", x: 2, y: 0, w: 3, h: 2 },
      { id: "yt-m", kind: "youtube", x: 5, y: 0, w: 3, h: 1 },
      { id: "bt-m", kind: "battery", x: 5, y: 1, w: 3, h: 1 },
      { id: "ml-m", kind: "mail", x: 0, y: 2, w: 2, h: 2 },
      { id: "rm-l", kind: "reminders", x: 2, y: 2, w: 3, h: 2 },
      { id: "ls-l", kind: "list", x: 5, y: 2, w: 3, h: 2, listId: M22_LISTS[0]!.id },
      { id: "st-m", kind: "statuses", x: 0, y: 4, w: 4, h: 1, title: "Checks" },
      { id: "nt-m", kind: "notifications", x: 4, y: 4, w: 4, h: 1 },
    ];
  } else if (pack === "empty") {
    (fx.layout as Record<string, unknown>).grid = { cols: 8, rows: 5 };
    fx.weather = null;
    fx.sys = null;
    fx.ai = null;
    fx.media = null;
    fx.obs = null;
    fx.twitch = null;
    fx.youtube = null;
    fx.battery = null;
    fx.mail = null;
    fx.reminders = { items: [] };
    fx.lists = { lists: [] };
    fx.statuses = [];
    fx.notifications = [];
    fx.layout.pages[0]!.items = [
      { id: "e-w", kind: "weather", x: 0, y: 0, w: 2, h: 2 },
      { id: "e-s", kind: "sys", x: 2, y: 0, w: 2, h: 2 },
      { id: "e-a", kind: "ai", x: 4, y: 0, w: 2, h: 2, show: "both" },
      { id: "e-m", kind: "media", x: 6, y: 0, w: 2, h: 2 },
      { id: "e-o", kind: "obs", x: 0, y: 2, w: 2, h: 2 },
      { id: "e-t", kind: "twitch", x: 2, y: 2, w: 2, h: 2 },
      { id: "e-y", kind: "youtube", x: 4, y: 2, w: 2, h: 2 },
      { id: "e-b", kind: "battery", x: 6, y: 2, w: 2, h: 2 },
      { id: "e-ml", kind: "mail", x: 0, y: 4, w: 2, h: 1 },
      { id: "e-rm", kind: "reminders", x: 2, y: 4, w: 2, h: 1 },
      { id: "e-ls", kind: "list", x: 4, y: 4, w: 2, h: 1 },
      { id: "e-nt", kind: "notifications", x: 6, y: 4, w: 2, h: 1 },
    ];
  } else if (pack === "m24-lyrics-sync" || pack === "m24-lyrics-plain") {
    const synced = pack === "m24-lyrics-sync";
    (fx.layout as Record<string, unknown>).grid = { cols: 6, rows: 3 };
    fx.layout.pages[0]!.items = [
      { id: "m4-1", kind: "media", x: 0, y: 0, w: 4, h: 2, prevAction: "media-prev", playAction: "media-play", nextAction: "media-next" },
      { id: "m4-w", kind: "weather", x: 4, y: 0, w: 2, h: 2 },
      { id: "m4-b1", kind: "button", x: 0, y: 2, w: 1, h: 1, label: "Mic", icon: "lucide:mic", actionId: "mic-toggle" },
      { id: "m4-b2", kind: "button", x: 1, y: 2, w: 1, h: 1, label: "Mute", icon: "lucide:volume-x", actionId: "out-mute" },
      { id: "m4-v", kind: "volume", x: 2, y: 2, w: 4, h: 1, upAction: "vol-up", downAction: "vol-down", muteAction: "vol-mute" },
    ];
    (fx.media as Record<string, unknown>).art = SAMPLE_ART;
    (fx.media as Record<string, unknown>).playing = false;
    (fx.media as Record<string, unknown>).positionSec = 14;
    fx.lyrics = {
      trackKey: "0123456789abcdef",
      synced,
      offsetMs: 500,
      lines: [
        { ...(synced ? { t: 4000 } : {}), text: "Waiting in a car" },
        { ...(synced ? { t: 9000 } : {}), text: "Waiting for a ride in the dark" },
        { ...(synced ? { t: 14000 } : {}), text: "The night city grows" },
        { ...(synced ? { t: 19000 } : {}), text: "Looking at the neon glow" },
        { ...(synced ? { t: 24000 } : {}), text: "Searching for the morning light" },
      ],
    };
  } else if (pack === "m24-weather-hourly") {
    (fx.layout as Record<string, unknown>).grid = { cols: 6, rows: 3 };
    fx.layout.pages[0]!.items = [
      { id: "wh-1", kind: "weather", x: 0, y: 0, w: 3, h: 2 },
      { id: "wh-c", kind: "clock", x: 3, y: 0, w: 3, h: 2 },
      { id: "wh-v", kind: "volume", x: 0, y: 2, w: 6, h: 1, upAction: "vol-up", downAction: "vol-down", muteAction: "vol-mute" },
    ];
    (fx.weather as Record<string, unknown>).hourly = [
      { time: "13:00", temp: 8, code: 1, isDay: true },
      { time: "14:00", temp: 9, code: 2, isDay: true },
      { time: "15:00", temp: 8, code: 2, isDay: true },
      { time: "16:00", temp: 6, code: 3, isDay: true },
      { time: "17:00", temp: 4, code: 61, isDay: false },
      { time: "18:00", temp: 3, code: 61, isDay: false },
    ];
  } else if (pack === "m24-rem-repeat") {
    (fx.layout as Record<string, unknown>).grid = { cols: 6, rows: 3 };
    fx.reminders = { items: M22_REMINDERS };
    fx.layout.pages[0]!.items = [
      { id: "rr-1", kind: "reminders", x: 0, y: 0, w: 4, h: 2 },
      { id: "rr-c", kind: "clock", x: 4, y: 0, w: 2, h: 2 },
      { id: "rr-v", kind: "volume", x: 0, y: 2, w: 6, h: 1, upAction: "vol-up", downAction: "vol-down", muteAction: "vol-mute" },
    ];
  } else if (pack === "m24-empty-deck") {
    fx.layout.pages = [{ id: "main", name: "Main", items: [] }];
  } else {
    (fx.media as Record<string, unknown>).art = SAMPLE_ART;
    fx.layout.pages[0]!.items.push({ id: "w-media", kind: "media", x: 2, y: 3, w: 3, h: 1, prevAction: "media-prev", playAction: "media-play", nextAction: "media-next" });
  }
  // OBS: connected, recording and live, so the widget shows no "Offline"; the sys graph gets a full history of a plausible curve.
  const now = Date.now();
  if (pack !== "empty") {
    if (fx.media && typeof fx.media === "object") (fx.media as Record<string, unknown>).positionAt = now;
    fx.obs = { connected: true, recording: true, recordStartedAt: now - 754_000, streaming: true, streamStartedAt: now - 3_910_000, scene: "Gaming", replayBuffer: false };
    resetSysHistory();
    const sys = fx.sys as { cpu: number; mem: { usedMb: number; totalMb: number } };
    for (let i = 0; i < 70; i++) {
      const cpu = Math.round(34 + 22 * Math.sin(i / 5) + 9 * Math.sin(i * 1.7) + (i % 7) * 1.5);
      const ramPct = 55 + 6 * Math.sin(i / 11) + i * 0.08;
      recordSys({ ...sys, cpu: Math.max(4, Math.min(96, cpu)), mem: { totalMb: 32000, usedMb: Math.round((ramPct / 100) * 32000) } } as never);
    }
  }
  if (scene === "deck") {
    await import("../../client/src/styles.css");
    localStorage.setItem("decka.glass.reduced.v1", "0");
    document.body.style.background = "#000";
    const { default: DeckScreen } = await import("../../client/src/screens/DeckScreen");
    setState({ conn: "online", state: fx as never });
    root.render(<DeckScreen />);
  } else {
    await import("../../agent/src/styles.css");
    if (q.get("wizard") !== null) localStorage.removeItem("decka.wizard.done.v1");
    else localStorage.setItem("decka.wizard.done.v1", "1");
    localStorage.setItem("decka.editor.tips.v1", "1");
    const schemeParam = q.get("scheme");
    if (schemeParam === "light" || schemeParam === "dark") {
      fx.layout.display = { ...fx.layout.display, windowTheme: schemeParam };
    }
    const layout = { version: 1, ...fx.layout };
    const answers: Record<string, unknown> = {
      get_layout: layout,
      save_layout: { ok: true, issues: [] },
      save_actions: { ok: true, issues: [] },
      save_config: { ok: true, issues: [] },
      list_devices: [],
      is_first_run: q.get("wizard") !== null,
      list_installed_apps: [
        { name: "Visual Studio Code", program: "C:\\Apps\\Code.exe", args: [] },
        { name: "Spotify", program: "C:\\Apps\\Spotify.exe", args: [] },
        { name: "OBS Studio", program: "C:\\Apps\\obs64.exe", args: [] },
        { name: "Discord", program: "C:\\Apps\\Discord.exe", args: [] },
      ],
      start_pairing: { code: "73914820", expiresAt: Date.now() + 120_000, url: "http://192.168.1.20:9247/pair#73914820", qrSvg: '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 40 40"><rect width="40" height="40" fill="#fff"/><rect x="4" y="4" width="12" height="12" fill="#111"/><rect x="24" y="4" width="12" height="12" fill="#111"/><rect x="4" y="24" width="12" height="12" fill="#111"/><rect x="20" y="20" width="8" height="8" fill="#111"/></svg>' },
      cancel_pairing: null,
      lyrics_get: false,
      lyrics_set: true,
      get_actions: demoActions(actionsFx.default as { version: number; actions: Record<string, unknown> }),
      get_update_status: { state: "idle" },
      get_status: { version: "0.22.0", focusApp: null, listening: { bind: "0.0.0.0", port: 9247 }, lanIps: ["192.168.1.20"], clients: [{ deviceId: "demo", name: "Phone", remote: "192.168.1.31", connectedAt: Date.now() }], usb: { enabled: false, adbFound: false, devices: [] }, configDir: "C:\\Users\\demo\\AppData\\Roaming\\Decka" },
      list_window_apps: ["code.exe", "chrome.exe", "obs64.exe"],
      get_templates: [],
      get_config: (await import("../../shared/fixtures/config.example.json")).default,
      reminders_get: M22_REMINDERS.map((r, i) => ({ ...r, done: i === 99 })).concat([{ id: "3000000000000009", text: "Old one", at: "2026-11-24T09:00", done: true }]),
      lists_get: M22_LISTS,
      memory_get: [{ id: "4000000000000001", text: "Aigerim is my wife", createdAt: 1 }, { id: "4000000000000002", text: "My project folder is on drive D", createdAt: 2 }],
      voice_dictation_get: { trigger: { kind: "doubleCtrl" }, mode: "clean", dictionary: ["Decka", "Qwen", "Almaty"] },
      voice_status: { state: "ready", hardware: { ramGb: 32 }, model: "standard", downloadGb: 3.2, hotkey: ["ctrl", "alt", "space"], keepLoaded: false, speak: false, phoneButton: true, mics: ["Microphone (USB)"] },
    };
    (window as never as { __TAURI_EVENT_PLUGIN_INTERNALS__: unknown }).__TAURI_EVENT_PLUGIN_INTERNALS__ = {
      unregisterListener: () => {},
    };
    (window as never as { __TAURI_INTERNALS__: unknown }).__TAURI_INTERNALS__ = {
      invoke: async (cmd: string) => {
        if (cmd in answers) return answers[cmd];
        console.log("unmocked:", cmd);
        return cmd.startsWith("plugin:event") ? 1 : null;
      },
      transformCallback: () => 1,
      metadata: { currentWindow: { label: "main" }, currentWebview: { label: "main", windowLabel: "main" } },
    };
    const { default: App } = await import("../../agent/src/App");
    root.render(<App />);
    if (q.get("go")) setTimeout(() => window.dispatchEvent(new CustomEvent("decka:go", { detail: { tab: q.get("go") } })), 500);
    if (q.get("wizard") !== null) setTimeout(() => window.dispatchEvent(new CustomEvent("decka:run-setup", { detail: { step: Number(q.get("wizard")) } })), 450);
  }
} else if (scene === "orb-bench") {
  const { ParticlesOrb } = await import("../../ui/src");
  document.body.style.background = "#0e1017";
  const orbState = (q.get("state") ?? "listening") as "idle" | "connecting" | "listening" | "thinking" | "speaking";
  const orbSize = Number(q.get("size") ?? 120);
  root.render(
    <div style={{ display: "flex", alignItems: "center", justifyContent: "center", minHeight: "100vh" }}>
      <ParticlesOrb state={orbState} size={orbSize} />
    </div>,
  );
} else if (scene === "island") {
  const { Island, ParticlesOrb } = await import("../../ui/src");
  const { useState } = await import("react");
  const themeMode = (q.get("theme") ?? "dark") as "light" | "dark";
  document.body.style.cssText =
    themeMode === "light"
      ? "margin:0;min-height:100vh;background:linear-gradient(135deg,#e8eef8,#f3e8ff);font-family:system-ui,-apple-system,sans-serif;"
      : "margin:0;min-height:100vh;background:radial-gradient(900px 600px at 20% 20%,#1e2b4f 0%,transparent 65%),radial-gradient(900px 600px at 80% 85%,#3b1c54 0%,transparent 65%),#0b0d14;font-family:system-ui,-apple-system,sans-serif;";
  if (q.get("interactive")) {
    const { t: tr } = await import("@decka/shared");
    function IslandDemoApp() {
      const [mode, setMode] = useState<"tab" | "compact" | "expanded">("expanded");
      const [phase, setPhase] = useState<"idle" | "loading" | "listening" | "thinking" | "speaking" | "error">("listening");
      const [level, setLevel] = useState(0.55);
      const [slow, setSlow] = useState(false);
      const [lyrics, setLyrics] = useState(true);
      const [showText, setShowText] = useState(true);
      const [demoTheme, setDemoTheme] = useState<"light" | "dark">(themeMode);
      const replayTransition = () => {
        setMode("expanded");
        setSlow(false);
        setPhase("listening");
        setShowText(false);
        window.setTimeout(() => {
          setPhase("done" as never);
          setShowText(true);
        }, 160);
      };
      const evt = {
        mode,
        lyrics: lyrics ? { playing: true, current: "Midnight City — M83", next: "Waiting in a car" } : null,
        voice: {
          active: true,
          phase,
          level,
          ...(showText ? {
            heard: q.get("lang") === "ru" ? "Что на экране?" : "What is on screen?",
            reply: q.get("lang") === "ru" ? "На экране открыт редактор кода и панель задач." : "A code editor and task panel are open on screen.",
            actions: ["copy", "insert", "speak", "again", "lang:en", "lang:ru"],
            chips: [{ label: q.get("lang") === "ru" ? "Открыть код" : "Open code", command: "open" }],
          } : {}),
          slow: showText && slow ? { etaSec: 12, elapsedSec: 4 } : null,
        },
      };
      return (
        <div style={{ padding: 24, display: "flex", flexDirection: "column", alignItems: "center", gap: 20, color: demoTheme === "light" ? "#14161c" : "#f4f6fa" }}>
          <div style={{ display: "flex", flexWrap: "wrap", gap: 8, alignItems: "center", fontSize: 12 }}>
            {(["tab", "compact", "expanded"] as const).map((m) => (
              <button key={m} onClick={() => setMode(m)}>{m}</button>
            ))}
            {(["idle", "loading", "listening", "thinking", "speaking", "error"] as const).map((p) => (
              <button key={p} onClick={() => setPhase(p)}>{p}</button>
            ))}
            <button onClick={replayTransition}>{tr("island.replay")}</button>
            <label>Level <input type="range" min={0} max={1} step={0.05} value={level} onChange={(e) => setLevel(Number(e.target.value))} /></label>
            <label><input type="checkbox" checked={showText} onChange={(e) => setShowText(e.target.checked)} /> Text</label>
            <label><input type="checkbox" checked={slow} onChange={(e) => setSlow(e.target.checked)} /> Slow</label>
            <label><input type="checkbox" checked={lyrics} onChange={(e) => setLyrics(e.target.checked)} /> Lyrics</label>
            <button onClick={() => setDemoTheme((d) => (d === "dark" ? "light" : "dark"))}>{demoTheme}</button>
          </div>
          <Island event={evt} theme={demoTheme} onAction={() => {}} onChipClick={() => {}} onStopSlow={() => setSlow(false)} />
          <ParticlesOrb state={phase === "loading" ? "connecting" : phase === "error" ? "idle" : phase} size={120} level={level} />
        </div>
      );
    }
    root.render(<IslandDemoApp />);
  } else {
    const mode = (q.get("mode") ?? "expanded") as "hidden" | "tab" | "compact" | "expanded";
    const phase = (q.get("state") ?? "listening") as "idle" | "loading" | "listening" | "thinking" | "speaking" | "error";
    const isRu = q.get("lang") === "ru";
    const showOrbRow = q.get("orbRow") === "1";
    if (showOrbRow) {
      const states = ["idle", "connecting", "listening", "thinking", "speaking"] as const;
      root.render(
        <div style={{ display: "flex", gap: 20, padding: 24, alignItems: "center", justifyContent: "center", minHeight: "100vh", boxSizing: "border-box" }}>
          {states.map((s) => (
            <div key={s} style={{ display: "flex", flexDirection: "column", alignItems: "center", gap: 8, color: themeMode === "light" ? "#14161c" : "#f4f6fa", fontSize: 12 }}>
              <ParticlesOrb state={s} size={120} level={0.65} />
              <span>{s}</span>
            </div>
          ))}
        </div>,
      );
    } else {
      const hasLyrics = q.get("lyrics") === "1";
      const isSlow = q.get("slow") === "1";
      const hasReply = q.get("reply") === "1";
      const isSolo = q.get("solo") === "1";
      const transParam = q.get("transition");
      const transitionPreview = transParam !== null ? Number(transParam) : undefined;
      const evt = {
        mode,
        lyrics: hasLyrics ? { playing: true, current: isRu ? "В полночном городе огни" : "Waiting in a car", next: isRu ? "Ждём рассвет у реки" : "Waiting for a ride in the dark" } : null,
        voice: q.get("voiceOff") === "1" ? null : {
          active: true,
          phase,
          level: Number(q.get("level") ?? 0.6),
          ...(isSolo ? {} : {
            heard: q.get("heard") ?? (isRu ? "Что на экране?" : "What is on my screen?"),
            reply: hasReply ? (q.get("replyText") ?? (isRu ? "На экране открыт список задач и терминал." : "A task list and terminal window are open on your screen.")) : undefined,
            actions: hasReply && phase !== "error" ? ["copy", "insert", "speak", "again", "lang:en", "lang:ru"] : undefined,
            slow: isSlow ? { etaSec: 12, elapsedSec: q.get("almost") === "1" ? 30 : 4 } : null,
            chips: hasReply && phase !== "error" ? [{ label: isRu ? "Копировать код" : "Copy snippet", command: "copy" }] : undefined,
          }),
        },
      };
      root.render(
        <div style={{ display: "flex", justifyContent: "center", paddingTop: 28 }}>
          <Island
            event={evt}
            theme={themeMode}
            hovered={q.get("hover") === "1"}
            transitionPreview={transitionPreview}
            hoveredAction={q.get("hoverBtn") ?? undefined}
            copiedPreview={q.get("copied") === "1"}
            langMenuOpenPreview={q.get("langMenu") === "1"}
            onAction={() => {}}
            onChipClick={() => {}}
            onStopSlow={() => {}}
          />
        </div>,
      );
    }
  }
} else if (scene === "service") {
  await import("../../client/src/styles.css");
  const { applyTheme } = await import("../../ui/src");
  applyTheme(document.documentElement, { theme: (q.get("theme") ?? "studio") as "studio", accent: "#7c5cff", wallpaper: "aurora" });
  const screen = q.get("screen") ?? "pair";
  if (screen === "pair" || screen === "qr") {
    const { default: PairScreen } = await import("../../client/src/screens/PairScreen");
    setState({ conn: "unpaired", state: null });
    root.render(<PairScreen showQrFrame={screen === "qr"} />);
  } else if (screen === "link") {
    const [stateFx] = await Promise.all([import("../../shared/fixtures/state.m15.json")]);
    const fx = JSON.parse(JSON.stringify(stateFx.default));
    fx.layout.display = { ...fx.layout.display, theme: q.get("theme") ?? "studio", language: q.get("lang") ?? "en" };
    const { default: DeckScreen } = await import("../../client/src/screens/DeckScreen");
    const { default: LinkCard } = await import("../../client/src/screens/LinkCard");
    setState({ conn: "offline", agentName: "DESKTOP-MAIN", state: fx as never });
    root.render(<><DeckScreen /><LinkCard reason="silent" /></>);
  } else if (screen === "update") {
    const { default: App } = await import("../../client/src/App");
    setState({ conn: "update_required", state: null });
    root.render(<App />);
  } else if (screen === "skeleton") {
    const { default: App } = await import("../../client/src/App");
    setState({ conn: "connecting", state: null });
    root.render(<App />);
  } else if (screen === "perm") {
    const [stateFx] = await Promise.all([import("../../shared/fixtures/state.m15.json")]);
    const fx = JSON.parse(JSON.stringify(stateFx.default));
    fx.layout.display = { ...fx.layout.display, theme: q.get("theme") ?? "studio", language: q.get("lang") ?? "en" };
    fx.ai = {
      claude: null, codex: null, sessions: [], history: [],
      pending: [{
        reqId: "0123456789abcdef", provider: "claude", tool: "Bash",
        summary: "git status && npm test -w client", project: "Decka",
        sessionId: "abc12345", expiresAt: Date.now() + 30_000,
        always: { kind: "command", value: "git status" },
      }],
    };
    const { default: DeckScreen } = await import("../../client/src/screens/DeckScreen");
    setState({ conn: "online", state: fx as never });
    root.render(<DeckScreen />);
  } else if (screen === "menu") {
    const [stateFx] = await Promise.all([import("../../shared/fixtures/state.m15.json")]);
    const fx = JSON.parse(JSON.stringify(stateFx.default));
    fx.layout.display = { ...fx.layout.display, theme: q.get("theme") ?? "studio", language: q.get("lang") ?? "en" };
    const { default: DeckScreen } = await import("../../client/src/screens/DeckScreen");
    const { default: PhoneMenu } = await import("../../client/src/menu/PhoneMenu");
    const { default: Toasts } = await import("../../client/src/deck/Toasts");
    setState({
      conn: "online",
      via: "lan",
      agentName: "DESKTOP-MAIN",
      agentVersion: "0.22.0",
      state: fx as never,
      toasts: [{ id: 1, level: "success", title: "Saved", body: "Layout updated on PC", until: Date.now() + 60_000 }],
    });
    root.render(
      <>
        <DeckScreen />
        <Toasts />
        <PhoneMenu reduced={false} onToggleReduced={() => {}} haptics={true} onToggleHaptics={() => {}} onUnpair={() => {}} onClose={() => {}} />
      </>,
    );
  }
} else {
  // The two bundles have their own style sheets that must not meet: only the scene's one is loaded.
  await import("../../client/src/styles.css");
  {
    document.body.style.background = "#000";
    localStorage.removeItem("decka.voicefab.v1");
    setState({ conn: "online", state: { vars: { "voice.ready": true }, voice: info, layout: { display: { theme: "amoled" }, pages: [] } } as never });
    root.render(
      <>
        <VoiceFab />
        <VoiceConfirmSheet />
      </>,
    );
  }
}
