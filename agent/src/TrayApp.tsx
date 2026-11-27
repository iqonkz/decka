import { useCallback, useEffect, useState } from "react";
import { t, type LanguageSetting, type TrayState } from "@decka/shared";
import { DeckEnvContext, EMPTY_DATA, TrayMiniPanel, useApplyLanguage } from "@decka/ui";
import { applyAccent } from "./accent";
import { call } from "./ipc";

const LOOK_EVERY_MS = 1000;
/** A key that asks "sure?" waits this long for the second press. */
const ARMED_MS = 3000;

/** The props of the panel from the answer of `tray_state`. */
export function panelData(state: TrayState | null, armed: string | null) {
  const media = state?.media;
  return {
    track: media?.title ? { title: media.title, artist: media.artist, art: media.art, playing: media.playing } : null,
    volume: state?.volume ?? 0,
    muted: state?.muted ?? false,
    micMuted: state?.micMuted ?? false,
    favorites: (state?.favorites ?? []).map((f) => ({ id: f.id, icon: f.icon, label: f.id === armed ? t("m32.tray_again") : f.label, active: f.active || f.id === armed })),
    nextEvent: state?.nextEvent ? { title: state.nextEvent.title, startTime: state.nextEvent.allDay ? t("m32.cal_all_day") : state.nextEvent.startTime, joinActionId: state.nextEvent.joinActionId } : null,
  };
}

/**
 * The window of the mini panel of the tray (T32.3, `index.html?tray=1`). It shows what the agent sends and names the key that
 * was pressed; the agent decides what that does. The window never becomes active; the agent hides and destroys it.
 */
export default function TrayApp() {
  const [state, setState] = useState<TrayState | null>(null);
  const [language, setLanguage] = useState<LanguageSetting | undefined>(undefined);
  const [theme, setTheme] = useState<"light" | "dark">(window.matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark");
  const [armed, setArmed] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  useApplyLanguage(language, navigator.language);
  const failed = useCallback((e: unknown) => setError(String(e)), []);

  const look = useCallback(() => {
    // A hidden panel waits for its end: it reads nothing.
    if (document.visibilityState === "hidden") return;
    call("tray_state").then(setState, failed);
  }, [failed]);

  useEffect(() => {
    for (const el of [document.documentElement, document.body]) {
      el.style.background = "transparent";
      el.style.overflow = "hidden";
    }
    document.body.style.margin = "0";
    call("get_layout").then((l) => {
      applyAccent(l.display.accent);
      setLanguage(l.display.language);
    }, failed);
    const media = window.matchMedia("(prefers-color-scheme: light)");
    const onTheme = () => setTheme(media.matches ? "light" : "dark");
    media.addEventListener("change", onTheme);
    look();
    const id = window.setInterval(look, LOOK_EVERY_MS);
    document.addEventListener("visibilitychange", look);
    return () => {
      media.removeEventListener("change", onTheme);
      document.removeEventListener("visibilitychange", look);
      window.clearInterval(id);
    };
  }, [look, failed]);

  useEffect(() => {
    if (armed === null) return;
    const id = window.setTimeout(() => setArmed(null), ARMED_MS);
    return () => window.clearTimeout(id);
  }, [armed]);

  /** Does it and looks again at once, so that the button shows the new state. */
  const run = (work: Promise<unknown>) => {
    setError(null);
    work.then(look, failed);
  };

  function press(actionId: string) {
    const key = state?.favorites.find((f) => f.id === actionId);
    if (key?.confirm && armed !== actionId) return setArmed(actionId);
    setArmed(null);
    run(call("tray_press", { actionId }));
  }

  return (
    <DeckEnvContext.Provider value={{ ...EMPTY_DATA, online: true, icons: state?.icons ?? {}, onPress: (id) => call("tray_press", { actionId: id }), onPressError: (e) => failed(e.message ?? e.code), onNavigate: () => {}, onDismiss: () => {} }}>
      <TrayMiniPanel
        theme={theme}
        {...panelData(state, armed)}
        error={error ?? undefined}
        onMedia={(kind) => run(call("media_control", { kind }))}
        onVolumeChange={(percent) => {
          setState((s) => (s ? { ...s, volume: percent } : s));
          run(call("tray_volume", { percent }));
        }}
        onToggleMute={() => run(call("tray_mute", { mic: false }))}
        onToggleMic={() => run(call("tray_mute", { mic: true }))}
        onFavorite={press}
        onJoinEvent={(actionId) => run(call("tray_press", { actionId }))}
        onOpenSettings={() => run(call("tray_open_settings"))}
      />
    </DeckEnvContext.Provider>
  );
}
