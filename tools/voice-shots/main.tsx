// Harness of tools/voice-shots.mjs: draws the real PC pill (agent) and the real phone button, bubble and confirmation sheet
// (client) in the states of DESIGN.md 4.12. Not part of the app.
import { createRoot } from "react-dom/client";
import { setLanguage, type Language } from "@decka/shared";
import VoicePill from "../../agent/src/VoicePill";
import VoiceFab from "../../client/src/voice/VoiceFab";
import VoiceConfirmSheet from "../../client/src/voice/VoiceConfirmSheet";
import { setState } from "../../client/src/store/store";

const q = new URLSearchParams(location.search);
const scene = q.get("scene") ?? "pill";
const state = (q.get("state") ?? "listening") as "idle" | "listening" | "loading" | "thinking" | "confirm" | "done" | "error";
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
};

const root = createRoot(document.getElementById("root")!);
if (scene === "deck" || scene === "settings") {
  // README pictures (T22.1): demo data from the shared fixtures, nothing personal.
  const [stateFx, actionsFx] = await Promise.all([import("../../shared/fixtures/state.m15.json"), import("../../shared/fixtures/actions.default.json")]);
  const fx = JSON.parse(JSON.stringify(stateFx.default)) as { layout: { display: Record<string, unknown>; pages: { items: unknown[] }[] }; [k: string]: unknown };
  fx.layout.display = { ...fx.layout.display, theme: q.get("theme") ?? "glass", wallpaper: q.get("wallpaper") ?? "aurora", orientation: "landscape" };
  fx.layout.pages[0]!.items.push({ id: "w-media", kind: "media", x: 2, y: 3, w: 3, h: 1, prevAction: "media-prev", playAction: "media-play", nextAction: "media-next" });
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
    const answers: Record<string, unknown> = { get_layout: layout, get_actions: actionsFx.default, get_update_status: { state: "idle" }, get_status: { running: true } };
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
  }
} else {
// The two bundles have their own style sheets that must not meet: only the scene's one is loaded.
if (scene === "pill") await import("../../agent/src/styles.css");
else await import("../../client/src/styles.css");
if (scene === "pill") {
  document.body.style.background = "#1b1b22";
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
