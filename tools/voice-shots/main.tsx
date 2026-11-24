// Harness of tools/voice-shots.mjs: draws the real PC pill (agent) and the real phone button, bubble and confirmation sheet
// (client) in the states of DESIGN.md 4.12. Not part of the app.
import { createRoot } from "react-dom/client";
import { setLanguage, type Language } from "@decka/shared";
import { recordSys, resetSysHistory } from "@decka/ui";
import VoicePill from "../../agent/src/VoicePill";
import VoiceFab from "../../client/src/voice/VoiceFab";
import VoiceConfirmSheet from "../../client/src/voice/VoiceConfirmSheet";
import { setState } from "../../client/src/store/store";

// The demo layout refers to a few actions that the default set does not have: simple stand-ins, so the editor shows no problems.
function demoActions(base: { version: number; actions: Record<string, unknown> }) {
  const stub = (name: string, op: string) => ({ name, type: "media", op });
  return { ...base, actions: { ...base.actions, "mic-toggle": stub("Mic on", "mute"), shuffle: stub("Shuffle", "next"), "out-mute": stub("Mute", "mute"), "obs-record": stub("Record", "play_pause"), "go-live": stub("Go live", "play_pause") } };
}

const q = new URLSearchParams(location.search);
const scene = q.get("scene") ?? "pill";
const state = (q.get("state") ?? "listening") as "idle" | "listening" | "loading" | "recognizing" | "thinking" | "confirm" | "done" | "error";
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
  // M22: the label of a dictation / of a look at the screen (DATA_MODEL.md 13.7).
  ...(q.get("activity") ? { activity: q.get("activity") } : {}),
};

// M22 demo data (nothing personal): the next reminders and two lists.
const M22_LISTS = [
  { id: "a1b2c3d4e5f60718", name: "Shopping", items: [{ id: "1000000000000001", text: "Milk", done: true }, { id: "1000000000000002", text: "Bread", done: false }, { id: "1000000000000003", text: "Eggs", done: false }, { id: "1000000000000004", text: "Coffee beans", done: false }] },
  { id: "0f1e2d3c4b5a6978", name: "Trip", items: [{ id: "2000000000000001", text: "Passport", done: false }, { id: "2000000000000002", text: "Charger", done: true }] },
];
const M22_REMINDERS = [
  { id: "3000000000000001", text: "Call the dentist", at: "2026-11-25T16:30" },
  { id: "3000000000000002", text: "Pay the rent", at: "2026-11-26T09:00" },
  { id: "3000000000000003", text: "Team meeting", at: "2026-11-27T11:00" },
];

const root = createRoot(document.getElementById("root")!);
if (scene === "deck" || scene === "settings") {
  // README pictures (T22.1): demo data from the shared fixtures, nothing personal.
  const [stateFx, actionsFx] = await Promise.all([import("../../shared/fixtures/state.m15.json"), import("../../shared/fixtures/actions.default.json")]);
  const fx = JSON.parse(JSON.stringify(stateFx.default)) as { layout: { display: Record<string, unknown>; pages: { items: unknown[] }[] }; [k: string]: unknown };
  fx.layout.display = { ...fx.layout.display, theme: q.get("theme") ?? "glass", wallpaper: q.get("wallpaper") ?? "aurora", orientation: "landscape" };
  if (q.get("lang")) fx.layout.display = { ...fx.layout.display, language: q.get("lang") };
  if (q.get("m22")) {
    // The phone widgets of T22.16 on a page of their own: reminders, two lists, the weather.
    fx.layout.pages[0]!.items = [
      { id: "w-rem", kind: "reminders", x: 0, y: 0, w: 3, h: 2 },
      { id: "w-l1", kind: "list", x: 3, y: 0, w: 2, h: 2, listId: M22_LISTS[0]!.id },
      { id: "w-l2", kind: "list", x: 5, y: 0, w: 2, h: 2, listId: M22_LISTS[1]!.id },
      { id: "w-l3", kind: "list", x: 0, y: 2, w: 2, h: 1, listId: "ffffffffffffffff" },
      { id: "w-rem2", kind: "reminders", x: 2, y: 2, w: 3, h: 1 },
    ];
    fx.reminders = { items: M22_REMINDERS };
    fx.lists = { lists: M22_LISTS };
  }
  fx.layout.pages[0]!.items.push({ id: "w-media", kind: "media", x: 2, y: 3, w: 3, h: 1, prevAction: "media-prev", playAction: "media-play", nextAction: "media-next" });
  // OBS: connected, recording and live, so the widget shows no "Offline"; the sys graph gets a full history of a plausible curve.
  const now = Date.now();
  fx.obs = { connected: true, recording: true, recordStartedAt: now - 754_000, streaming: true, streamStartedAt: now - 3_910_000, scene: "Gaming", replayBuffer: false };
  resetSysHistory();
  const sys = fx.sys as { cpu: number; mem: { usedMb: number; totalMb: number } };
  for (let i = 0; i < 70; i++) {
    const cpu = Math.round(34 + 22 * Math.sin(i / 5) + 9 * Math.sin(i * 1.7) + (i % 7) * 1.5);
    const ramPct = 55 + 6 * Math.sin(i / 11) + i * 0.08;
    recordSys({ ...sys, cpu: Math.max(4, Math.min(96, cpu)), mem: { totalMb: 32000, usedMb: Math.round((ramPct / 100) * 32000) } } as never);
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
    localStorage.setItem("decka.wizard.done.v1", "1");
    localStorage.setItem("decka.editor.tips.v1", "1");
    const layout = { version: 1, ...fx.layout };
    const answers: Record<string, unknown> = { get_layout: layout, get_actions: demoActions(actionsFx.default as { version: number; actions: Record<string, unknown> }), get_update_status: { state: "idle" }, get_status: { version: "0.20.1", focusApp: null, listening: { bind: "0.0.0.0", port: 9247 }, lanIps: ["192.168.1.20"], clients: [{ deviceId: "demo", name: "Phone", remote: "192.168.1.31", connectedAt: Date.now() }], usb: { enabled: false, adbFound: false, devices: [] }, configDir: "C:\\Users\\demo\\AppData\\Roaming\\Decka" }, list_window_apps: ["code.exe", "chrome.exe"], get_templates: [], get_config: (await import("../../shared/fixtures/config.example.json")).default,
      // M22 windows: Reminders, Dictation and Memory.
      reminders_get: M22_REMINDERS.map((r, i) => ({ ...r, done: i === 99 })).concat([{ id: "3000000000000009", text: "Old one", at: "2026-11-24T09:00", done: true }]),
      lists_get: M22_LISTS,
      memory_get: [{ id: "4000000000000001", text: "Aigerim is my wife", createdAt: 1 }, { id: "4000000000000002", text: "My project folder is on drive D", createdAt: 2 }],
      voice_dictation_get: { trigger: { kind: "doubleCtrl" }, mode: "clean", dictionary: ["Decka", "Qwen", "Almaty"] },
      voice_status: { state: "ready", hardware: { ramGb: 32 }, model: "standard", downloadGb: 3.2, hotkey: ["ctrl", "alt", "space"], keepLoaded: false, speak: false, phoneButton: true, mics: ["Microphone (USB)"] } };
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
    // `go=reminders` / `go=voice` opens that page of More (the same event the app uses itself).
    if (q.get("go")) setTimeout(() => window.dispatchEvent(new CustomEvent("decka:go", { detail: { tab: q.get("go") } })), 500);
  }
} else {
// The two bundles have their own style sheets that must not meet: only the scene's one is loaded.
if (scene === "pill") await import("../../agent/src/styles.css");
else await import("../../client/src/styles.css");
if (scene === "pill") {
  document.body.style.background = "#1b1b22";
  // `desk=1` (T22.3): a neutral desktop (gradient, no logos) with the pill near the top of the screen.
  if (q.get("desk")) {
    document.body.style.cssText += ";margin:0;min-height:100vh;display:flex;align-items:flex-start;justify-content:center;padding-top:150px;box-sizing:border-box;background:radial-gradient(1200px 700px at 20% 15%,#3b4a8c 0%,transparent 60%),radial-gradient(1000px 700px at 85% 90%,#7a3b8c 0%,transparent 55%),linear-gradient(135deg,#141a33,#241437);";
  }
  if (q.get("desk")) document.getElementById("root")!.style.zoom = "2";
  // `pad=1`: the pill sits in the middle of a canvas that the shooter crops to the pill plus 40 px on every side.
  if (q.get("pad")) document.body.style.cssText += ";display:flex;align-items:center;justify-content:center;";
  root.render(<VoicePill initialData={{ ...info, level: Number(q.get("level") ?? 0.6), hotkey: "Ctrl+Alt+Space" } as never} />);
} else {
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
