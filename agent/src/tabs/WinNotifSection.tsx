import { useEffect, useState } from "react";
import { t, type ConfigFile, type WindowsNotificationsStatus } from "@decka/shared";
import { call } from "../ipc";
import Banner from "../ui/Banner";

/**
 * `More → Integrations → Windows notifications` (DATA_MODEL.md 10.4): the switch (Windows asks its own question the first time),
 * what Windows answered, and the apps whose notifications are not sent (`Don't show from:`).
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

  async function save(next: { enabled: boolean; exclude: string[] }) {
    setError(null);
    const fresh = await call("get_config");
    const result = await call("save_config", { config: { ...fresh, windowsNotifications: next } });
    if (!result.ok) return setError(result.issues[0]?.message ?? "");
    await onSaved();
  }

  const enabled = saved?.enabled ?? false;
  const exclude = saved?.exclude ?? [];
  // Apps already excluded stay in the list even when none of them sent a toast since the start.
  const apps = [...new Set([...(status?.apps ?? []), ...exclude])].sort((a, b) => a.localeCompare(b));
  const state = !enabled ? null : status?.access === "allowed" ? t("winnotif.allowed") : status?.access === "denied" ? t("winnotif.denied") : t("winnotif.unknown");

  return (
    <section className="card" id="winnotif">
      <h3>{t("winnotif.title")}</h3>
      <div className="row">
        <label className="check grow">
          <input type="checkbox" checked={enabled} onChange={(e) => void save({ enabled: e.target.checked, exclude })} /> {t("winnotif.enable")}
        </label>
        {state && <strong className={status?.access === "allowed" ? "ok-text" : "warn-text"}>{state}</strong>}
      </div>
      {enabled && (
        <div>
          <div className="muted">{t("winnotif.exclude")}</div>
          {apps.length === 0 && <small className="muted block">{t("winnotif.no_apps")}</small>}
          {apps.map((app) => (
            <label className="check block" key={app}>
              <input
                type="checkbox"
                checked={exclude.includes(app)}
                onChange={(e) => void save({ enabled, exclude: e.target.checked ? [...exclude, app].slice(0, 50) : exclude.filter((x) => x !== app) })}
              />{" "}
              {app}
            </label>
          ))}
        </div>
      )}
      {error && <Banner kind="error">{error}</Banner>}
      <small className="muted block">{t("winnotif.hint")}</small>
    </section>
  );
}
