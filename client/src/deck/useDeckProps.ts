import { useCallback, useMemo } from "react";
import { EMPTY_DATA, pressErrorTitle, type DeckCallbacks, type DeckData } from "@decka/ui";
import { dismiss, press } from "../app/controller";
import { lastTouchWokeScreen } from "../display/idle";
import { hapticEvent } from "../platform/haptics";
import { getState, navigateManually, openPopup, pushToast, useAppState } from "../store/store";

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
  const icons = useAppState((s) => s.state?.icons);

  const data: DeckData = useMemo(
    () => ({
      online,
      vars: vars ?? EMPTY_DATA.vars,
      sys: sys ?? null,
      statuses: statuses ?? EMPTY_DATA.statuses,
      notifications: notifications ?? EMPTY_DATA.notifications,
      media: media ?? null,
      icons: icons ?? NO_ICONS,
    }),
    [online, vars, sys, statuses, notifications, media, icons],
  );

  const onPressError = useCallback((e: { code: string; message?: string }) => {
    hapticEvent({ type: "press_result", ok: false });
    pushToast({ level: "error", title: pressErrorTitle(e), ms: ERROR_TOAST_MS });
  }, []);
  const onNavigate = useCallback((pageId: string) => {
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

  return { data, onPress: press, onPressError, onNavigate, onDismiss: dismiss, onOpenPopup, onFeedback };
}
