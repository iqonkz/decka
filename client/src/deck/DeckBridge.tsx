import { useCallback, useMemo } from "react";
import type { Page } from "@decka/shared";
import { EMPTY_DATA, Grid, pressErrorTitle, type DeckData } from "@decka/ui";
import { dismiss, press } from "../app/controller";
import { getState, navigateManually, pushToast, useAppState } from "../store/store";

const ERROR_TOAST_MS = 4000;
const NO_ICONS: Record<string, string> = {};

/** The deck for one page, fed from the store and wired to the controller (the only place where the two meet). */
export default function DeckBridge({ page, grid }: { page: Page; grid: { cols: number; rows: number } }) {
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

  const onPressError = useCallback((e: { code: string; message?: string }) => pushToast({ level: "error", title: pressErrorTitle(e), ms: ERROR_TOAST_MS }), []);
  const onNavigate = useCallback((pageId: string) => {
    const index = getState().state?.layout.pages.findIndex((p) => p.id === pageId) ?? -1;
    if (index >= 0) navigateManually(index);
  }, []);

  return <Grid page={page} grid={grid} data={data} onPress={press} onPressError={onPressError} onNavigate={onNavigate} onDismiss={dismiss} />;
}
