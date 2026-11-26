import { useEffect, useRef, useState } from "react";
import { t, tVoice, type IslandEvent, type LanguageSetting } from "@decka/shared";
import { GuidePanel, Island, useApplyLanguage, type GuideStepEvent } from "@decka/ui";
import { call, listen } from "./ipc";

const HIDDEN: IslandEvent = { mode: "hidden" };

/** A reply or a question of the agent as text: the i18n key of a fixed phrase when there is one, else the text itself. */
function localized(event: IslandEvent): IslandEvent {
  const v = event.voice;
  if (!v) return event;
  const reply = v.phase === "confirm" ? tVoice(v.confirm?.key, v.confirm?.args, v.confirm?.text) : tVoice(v.replyKey, v.replyArgs, v.reply);
  // Hint chips carry an i18n key as their label.
  const chips = v.chips?.map((c) => ({ ...c, label: c.label.startsWith("voice.chip.") ? (tVoice(c.label, undefined, c.label) ?? c.label) : c.label }));
  return { ...event, voice: { ...v, reply: reply || v.reply, chips } };
}

/** `?theme=light|dark` (snapshots only): the colour scheme of the island regardless of the system. */
function forcedTheme(): "light" | "dark" | undefined {
  const v = new URLSearchParams(window.location.search).get("theme");
  return v === "light" || v === "dark" ? v : undefined;
}

/** `?glass=acrylic`: the window carries the system acrylic and the page draws only a light tint (T26.F.3). */
const GLASS = new URLSearchParams(window.location.search).get("glass") === "acrylic" ? "acrylic" : "css";

/** The panel with the language menu while it is open (the menu drops below the panel): the window grows to hold both (T26.F.9). */
export function unionRect(a: DOMRect | undefined, b: DOMRect | undefined): { x: number; y: number; width: number; height: number } | undefined {
  if (!a) return b;
  if (!b || b.width === 0 || b.height === 0) return a;
  const x = Math.min(a.x, b.x);
  const y = Math.min(a.y, b.y);
  return { x, y, width: Math.max(a.right, b.right) - x, height: Math.max(a.bottom, b.bottom) - y };
}

/**
 * The root of the island window (`index.html?island=1`, T26.4/T26.R): draws the state the agent sends (`island_state`), reports the
 * rectangle of the visible panel (`island_rect`) so that the rest of the window lets clicks through, and names the action a button
 * stands for. What an action does is decided by the PC.
 */
export default function IslandApp() {
  const [event, setEvent] = useState<IslandEvent>(HIDDEN);
  const [language, setLanguage] = useState<LanguageSetting | undefined>(undefined);
  const [theme, setTheme] = useState<"light" | "dark">(forcedTheme() ?? (window.matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark"));
  const host = useRef<HTMLDivElement>(null);
  // T27.5: the guide in progress (goal, step, text); its buttons name the control, the PC decides what it means.
  const [guide, setGuide] = useState<GuideStepEvent | null>(null);
  useApplyLanguage(language, navigator.language);

  useEffect(() => {
    document.body.style.background = "transparent";
    document.documentElement.style.background = "transparent";
    call("get_layout").then(
      (l) => {
        document.documentElement.style.setProperty("--accent", l.display.accent ?? "#7c5cff");
        setLanguage(l.display.language);
      },
      () => {},
    );
    const media = window.matchMedia("(prefers-color-scheme: light)");
    const onTheme = () => setTheme(forcedTheme() ?? (media.matches ? "light" : "dark"));
    media.addEventListener("change", onTheme);
    let off: (() => void) | undefined;
    let live = true;
    void listen<IslandEvent>("island_state", (e) => setEvent(e.payload)).then((fn) => {
      if (live) off = fn;
      else fn();
    });
    let offGuide: (() => void) | undefined;
    void listen<GuideStepEvent>("guide_step", (e) => setGuide(e.payload.active ? e.payload : null)).then((fn) => {
      if (live) offGuide = fn;
      else fn();
    });
    return () => {
      live = false;
      off?.();
      offGuide?.();
      media.removeEventListener("change", onTheme);
    };
  }, []);

  // The visible panel is the first element of the host that has a size.
  useEffect(() => {
    const el = host.current;
    if (!el) return;
    const report = () => {
      // T27.F.2: a hidden window has no panel to measure; no timers or IPC while it is hidden.
      if (document.visibilityState === "hidden") return;
      const panel = el.querySelector<HTMLElement>(".island-panel, .island-tab-zone, .m27-guide-card");
      const r = unionRect(panel?.getBoundingClientRect(), el.querySelector<HTMLElement>(".island-lang-menu")?.getBoundingClientRect());
      void call("island_rect", { x: r?.x ?? 0, y: r?.y ?? 0, width: r?.width ?? 0, height: r?.height ?? 0 }).catch(() => {});
    };
    report();
    const ro = new ResizeObserver(report);
    ro.observe(el);
    const mo = new MutationObserver(report);
    mo.observe(el, { childList: true, subtree: true });
    // The panel changes size during its 220 ms transition (the hover buttons widen it): report often while it runs.
    const timer = window.setInterval(report, 250);
    let fast: number | undefined;
    const onTransition = () => {
      window.clearInterval(fast);
      fast = window.setInterval(report, 40);
      window.setTimeout(() => window.clearInterval(fast), 320);
    };
    el.addEventListener("transitionrun", onTransition);
    document.addEventListener("visibilitychange", report);
    return () => {
      document.removeEventListener("visibilitychange", report);
      ro.disconnect();
      mo.disconnect();
      window.clearInterval(timer);
      window.clearInterval(fast);
      el.removeEventListener("transitionrun", onTransition);
    };
  }, []);

  const shown = localized(event);
  const confirmId = event.voice?.phase === "confirm" ? event.voice.confirmId : undefined;
  return (
    <div ref={host} data-island-glass={GLASS} style={{ width: "100%", boxSizing: "border-box", padding: 24 }}>
      <Island
        event={shown}
        theme={theme}
        onMediaControl={(kind) => void call("media_control", { kind }).catch(() => {})}
        onAction={(kind) => void call("voice_action", { kind: kind as never }).catch(() => {})}
        onChipClick={(command) => void call("voice_chip", { command: command as never }).catch(() => {})}
        onStopSlow={() => void call("voice_cancel").catch(() => {})}
      />
      <GuidePanel
        guide={guide}
        theme={theme}
        onBack={() => void call("guide_control", { kind: "back" }).catch(() => {})}
        onStop={() => void call("guide_control", { kind: "stop" }).catch(() => {})}
      />
      {confirmId && (
        <div className="island-host" data-island-theme={theme} style={{ marginTop: 8, gap: 8 }}>
          <button type="button" className="island-action-btn" onClick={() => void call("voice_confirm", { id: confirmId, yes: true })}>
            {t("voice.yes")}
          </button>
          <button type="button" className="island-action-btn" onClick={() => void call("voice_confirm", { id: confirmId, yes: false })}>
            {t("voice.no")}
          </button>
        </div>
      )}
    </div>
  );
}