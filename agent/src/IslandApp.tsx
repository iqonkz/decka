import { useEffect, useRef, useState } from "react";
import { t, tVoice, type IslandEvent, type LanguageSetting } from "@decka/shared";
import { Island, useApplyLanguage } from "@decka/ui";
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

/**
 * The root of the island window (`index.html?island=1`, T26.4/T26.R): draws the state the agent sends (`island_state`), reports the
 * rectangle of the visible panel (`island_rect`) so that the rest of the window lets clicks through, and names the action a button
 * stands for. What an action does is decided by the PC.
 */
export default function IslandApp() {
  const [event, setEvent] = useState<IslandEvent>(HIDDEN);
  const [language, setLanguage] = useState<LanguageSetting | undefined>(undefined);
  const [theme, setTheme] = useState<"light" | "dark">(window.matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark");
  const host = useRef<HTMLDivElement>(null);
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
    const onTheme = () => setTheme(media.matches ? "light" : "dark");
    media.addEventListener("change", onTheme);
    let off: (() => void) | undefined;
    let live = true;
    void listen<IslandEvent>("island_state", (e) => setEvent(e.payload)).then((fn) => {
      if (live) off = fn;
      else fn();
    });
    return () => {
      live = false;
      off?.();
      media.removeEventListener("change", onTheme);
    };
  }, []);

  // The visible panel is the first element of the host that has a size.
  useEffect(() => {
    const el = host.current;
    if (!el) return;
    const report = () => {
      const panel = el.querySelector<HTMLElement>(".island-panel, .island-tab-zone");
      const r = panel?.getBoundingClientRect();
      void call("island_rect", { x: r?.x ?? 0, y: r?.y ?? 0, width: r?.width ?? 0, height: r?.height ?? 0 }).catch(() => {});
    };
    report();
    const ro = new ResizeObserver(report);
    ro.observe(el);
    const mo = new MutationObserver(report);
    mo.observe(el, { childList: true, subtree: true });
    // The panel changes size during its 220 ms transition.
    const timer = window.setInterval(report, 250);
    return () => {
      ro.disconnect();
      mo.disconnect();
      window.clearInterval(timer);
    };
  }, []);

  const shown = localized(event);
  const confirmId = event.voice?.phase === "confirm" ? event.voice.confirmId : undefined;
  return (
    <div ref={host} style={{ width: "100%", boxSizing: "border-box", padding: 24 }}>
      <Island
        event={shown}
        theme={theme}
        onAction={(kind) => void call("voice_action", { kind: kind as never }).catch(() => {})}
        onChipClick={(command) => void call("voice_chip", { command: command as never }).catch(() => {})}
        onStopSlow={() => void call("voice_cancel").catch(() => {})}
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