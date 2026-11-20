import { useEffect, useState } from "react";
import { effectivePrivacy, t, type ConfigFile, type NotifPrivacy, type WindowsNotificationsConfig, type WindowsNotificationsStatus } from "@decka/shared";
import { call } from "../ipc";
import Banner from "../ui/Banner";

type Choice = "full" | "app" | "hide";

/**
 * `More → Integrations → Windows notifications` (DATA_MODEL.md 10.4, 11.5): the switch (Windows asks its own question the first time),
 * what Windows answered, and for every program how much goes to the phone: `Full text`, `App name only` or `Don't show`.
 */
export default function WinNotifSection({ config, onSaved }: { config: ConfigFile; onSaved: () => Promise<void> }) {
  const saved = config.windowsNotifications;
  const [status, setStatus] = useState<WindowsNotificationsStatus | null>(null);
  const [error, setError] = useState<string | null>(null);

  // The access and the apps seen so far are asked for while the section is on screen.
  useEffect(() => {
    let live = true;
    const ask = () => void call("windows_notifications_status").then((s) => live && setStatus(s), () => live && setStatus(null));
    ask();
    const timer = window.setInterval(ask, 3000);
    return () => {
      live = false;
      window.clearInterval(timer);
    };
  }, [saved?.enabled]);

  async function save(next: WindowsNotificationsConfig) {
    setError(null);
    const fresh = await call("get_config");
    const result = await call("save_config", { config: { ...fresh, windowsNotifications: next } });
    if (!result.ok) return setError(result.issues[0]?.message ?? "");
    await onSaved();
  }

  const enabled = saved?.enabled ?? false;
  const exclude = saved?.exclude ?? [];
  const privacy: NotifPrivacy = saved?.privacy ?? { default: "full", apps: {} };
  // Apps already excluded stay in the list even when none of them sent a toast since the start.
  const apps = [...new Set([...(status?.apps ?? []), ...exclude])].sort((a, b) => a.localeCompare(b));
  const state = !enabled ? null : status?.access === "allowed" ? t("winnotif.allowed") : status?.access === "denied" ? t("winnotif.denied") : t("winnotif.unknown");
  const choiceOf = (app: string): Choice => (exclude.some((e) => e.toLowerCase() === app.toLowerCase()) ? "hide" : effectivePrivacy(app, privacy));

  function choose(app: string, choice: Choice) {
    const others = exclude.filter((e) => e.toLowerCase() !== app.toLowerCase());
    const nextApps = { ...privacy.apps };
    for (const name of Object.keys(nextApps)) if (name.toLowerCase() === app.toLowerCase()) delete nextApps[name];
    if (choice !== "hide") nextApps[app] = choice; // an explicit choice also overrides the built-in messenger list
    void save({ enabled, exclude: choice === "hide" ? [...others, app].slice(0, 50) : others, privacy: { default: privacy.default, apps: nextApps } });
  }

  return (
    <section className="card" id="winnotif">
      <h3>{t("winnotif.title")}</h3>
      <div className="row">
        <label className="check grow">
          <input type="checkbox" checked={enabled} onChange={(e) => void save({ ...saved, enabled: e.target.checked, exclude })} /> {t("winnotif.enable")}
        </label>
        {state && <strong className={status?.access === "allowed" ? "ok-text" : "warn-text"}>{state}</strong>}
      </div>
      {enabled && (
        <div>
          <div className="muted">{t("winnotif.exclude")}</div>
          {apps.length === 0 && <small className="muted block">{t("winnotif.no_apps")}</small>}
          {apps.map((app) => (
            <div className="row" key={app}>
              <span className="grow">{app}</span>
              <select value={choiceOf(app)} onChange={(e) => choose(app, e.target.value as Choice)}>
                <option value="full">{t("winnotif.mode.full")}</option>
                <option value="app">{t("winnotif.mode.app")}</option>
                <option value="hide">{t("winnotif.mode.hide")}</option>
              </select>
            </div>
          ))}
        </div>
      )}
      {error && <Banner kind="error">{error}</Banner>}
      <small className="muted block">{t("winnotif.hint")}</small>
    </section>
  );
}
