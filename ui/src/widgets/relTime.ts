import { t } from "@decka/shared";

/** "now" under a minute, then "<n>m", "<n>h", "<n>d" (in the interface language). */
export function relTime(ts: number, now: number): string {
  const seconds = Math.max(0, Math.floor((now - ts) / 1000));
  if (seconds < 60) return t("time.now");
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return t("time.minutes", { n: minutes });
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return t("time.hours", { n: hours });
  return t("time.days", { n: Math.floor(hours / 24) });
}
