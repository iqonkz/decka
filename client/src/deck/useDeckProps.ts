import { useCallback, useMemo } from "react";
import { t, BACK_PAGE } from "@decka/shared";
import { EMPTY_DATA, pressErrorTitle, type DeckCallbacks, type DeckData } from "@decka/ui";
import { dismiss, openNotification, press } from "../app/controller";
import { lastTouchWokeScreen } from "../display/idle";
import { hapticEvent } from "../platform/haptics";
import { getState, goBack, navigateManually, openPopup, pushToast, openPermHistory, reopenPerm, signalAttention, useAppState } from "../store/store";

const ERROR_TOAST_MS = 4000;
const NO_ICONS: Record<string, string> = {};

/** Everything `@decka/ui` needs from the app: the data of the current state and the callbacks wired to the controller. */
export function useDeckProps(): { data: DeckData } & Required<DeckCallbacks> {
  const online = useAppState((s) => s.conn === "online");
  const vars = useAppState((s) => s.state?.vars);
  const sys = useAppState((s) => s.state?.sys);
  const statuses = useAppState((s) => s.state?.statuses);
  const notifications = useAppState((s) => s.state?.notifications);
  const media = useAppState((s) => s.state?.media);
  const weather = useAppState((s) => s.state?.weather);
  const obs = useAppState((s) => s.state?.obs);
  const ai = useAppState((s) => s.state?.ai);
  const twitch = useAppState((s) => s.state?.twitch);
  const youtube = useAppState((s) => s.state?.youtube);
  const battery = useAppState((s) => s.state?.battery);
  const mail = useAppState((s) => s.state?.mail);
  const icons = useAppState((s) => s.state?.icons);
  const allPages = useAppState((s) => s.state?.layout.pages);
  const focusApp = useAppState((s) => s.state?.focus?.app);
  // Only the pages that follow an app matter to the context widget; the list is stable until one of them changes.
  const appPages = useMemo(() => allPages?.filter((p) => p.apps !== undefined && p.apps.length > 0), [allPages]);
  const keyFace = useAppState((s) => s.state?.layout.display.keyFace);
  const keyLabels = useAppState((s) => s.state?.layout.display.keyLabels);

  const data: DeckData = useMemo(
    () => ({
      online,
      vars: vars ?? EMPTY_DATA.vars,
      sys: sys ?? null,
      statuses: statuses ?? EMPTY_DATA.statuses,
      notifications: notifications ?? EMPTY_DATA.notifications,
      media: media ?? null,
      weather: weather ?? null,
      obs: obs ?? null,
      ai: ai ?? null,
      twitch: twitch ?? null,
      youtube: youtube ?? null,
      battery: battery ?? null,
      mail: mail ?? null,
      icons: icons ?? NO_ICONS,
      ...(appPages !== undefined ? { pages: appPages } : {}),
      ...(focusApp !== undefined ? { focusApp } : {}),
      ...(keyFace !== undefined ? { keyFace } : {}),
      ...(keyLabels !== undefined ? { keyLabels } : {}),
    }),
    [online, vars, sys, statuses, notifications, media, weather, obs, ai, twitch, youtube, battery, mail, icons, appPages, focusApp, keyFace, keyLabels],
  );

  const onPressError = useCallback((e: { code: string; message?: string }) => {
    hapticEvent({ type: "press_result", ok: false });
    pushToast({ level: "error", title: pressErrorTitle(e), ms: ERROR_TOAST_MS });
  }, []);
  const onTimerDone = useCallback(() => {
    pushToast({ level: "warning", title: t("timer.done") });
    signalAttention("warning"); // a warning wakes the screen from dimming and the screensaver
  }, []);
  const onNavigate = useCallback((pageId: string) => {
    if (pageId === BACK_PAGE) {
      hapticEvent({ type: "page_change", manual: true });
      goBack();
      return;
    }
    const index = getState().state?.layout.pages.findIndex((p) => p.id === pageId) ?? -1;
    if (index >= 0) {
      hapticEvent({ type: "page_change", manual: true });
      navigateManually(index);
    }
  }, []);
  const onFeedback = useCallback((moment: "touch" | "confirm") => {
    hapticEvent(moment === "touch" ? { type: "touch", wokeScreen: lastTouchWokeScreen() } : { type: "confirm" });
  }, []);
  const onOpenPopup = useCallback((popupId: string, anchor: { x: number; y: number; width: number; height: number }) => {
    hapticEvent({ type: "popup_open" });
    openPopup(popupId, anchor);
  }, []);

  return { data, onPress: press, onPressError, onNavigate, onDismiss: dismiss, onOpenNotification: openNotification, onOpenPopup, onFeedback, onTimerDone, onOpenPerm: reopenPerm, onOpenPermHistory: openPermHistory };
}
