import { useEffect, useState } from "react";
import { t, type ConfigFile, type ObsStatus } from "@decka/shared";
import { call } from "../ipc";
import Banner from "../ui/Banner";
import Button from "../ui/Button";

/** `More → Integrations → OBS Studio` (DATA_MODEL.md 6.4): switch, port, password, connection state, how to turn the server on in OBS. */
export default function ObsSection({ config, onSaved }: { config: ConfigFile; onSaved: () => Promise<void> }) {
  const saved = config.obs;
  const [enabled, setEnabled] = useState(saved?.enabled ?? false);
  const [port, setPort] = useState(String(saved?.port ?? 4455));
  const [editing, setEditing] = useState(false);
  const [password, setPassword] = useState("");
  const [status, setStatus] = useState<ObsStatus | null>(null);
  const [error, setError] = useState<string | null>(null);

  // The state of the connection is asked for while the switch is on.
  useEffect(() => {
    if (!saved?.enabled) return setStatus(null);
    let live = true;
    const ask = () => void call("obs_status").then((s) => live && setStatus(s), () => live && setStatus(null));
    ask();
    const timer = window.setInterval(ask, 2000);
    return () => {
      live = false;
      window.clearInterval(timer);
    };
  }, [saved?.enabled, saved?.port, saved?.passwordSet]);

  async function save(next: { enabled: boolean; port: string }) {
    setError(null);
    const fresh = await call("get_config");
    const obs = { enabled: next.enabled, port: Number(next.port) || 4455, ...(fresh.obs?.passwordSet ? { passwordSet: true } : {}) };
    const result = await call("save_config", { config: { ...fresh, obs } });
    if (!result.ok) return setError(result.issues[0]?.message ?? "");
    await onSaved();
  }

  async function savePassword() {
    setError(null);
    try {
      await call("set_obs_password", { password });
      setPassword("");
      setEditing(false);
      await onSaved();
    } catch (e) {
      setError(String(e));
    }
  }

  const state = !saved?.enabled ? null : status === null ? null : status.connected ? t("obsset.connected") : status.error === "Wrong password" ? t("obsset.wrong_password") : t("obsset.not_running");
  return (
    <section className="card" id="obs">
      <h3>{t("obsset.title")}</h3>
      <label className="check">
        <input
          type="checkbox"
          checked={enabled}
          onChange={(e) => {
            setEnabled(e.target.checked);
            void save({ enabled: e.target.checked, port });
          }}
        />{" "}
        {t("obsset.enable")}
      </label>
      <div className="row">
        <label className="field-row">
          <span>{t("obsset.port")}</span>
          <input inputMode="numeric" value={port} onChange={(e) => setPort(e.target.value.replace(/\D/g, "").slice(0, 5))} onBlur={() => void save({ enabled, port })} style={{ width: 90 }} />
        </label>
        <span className="grow" />
        {state && <strong className={status?.connected ? "ok-text" : "warn-text"}>{state}</strong>}
      </div>
      {!editing ? (
        <div className="row">
          <Button onClick={() => setEditing(true)}>{t("obsset.set_password")}</Button>
          <span className="muted">{saved?.passwordSet ? t("obsset.password_set") : t("obsset.no_password")}</span>
        </div>
      ) : (
        <div className="row">
          <input type="password" autoFocus autoComplete="off" className="grow" aria-label={t("obsset.password")} placeholder={t("obsset.password")} value={password} onChange={(e) => setPassword(e.target.value)} onKeyDown={(e) => e.key === "Enter" && void savePassword()} />
          <Button variant="primary" onClick={() => void savePassword()}>{password === "" ? t("obsset.remove") : t("common.save")}</Button>
          <Button onClick={() => { setEditing(false); setPassword(""); }}>{t("common.cancel")}</Button>
        </div>
      )}
      {error && <Banner kind="error">{error}</Banner>}
      <small className="muted block">{t("obsset.hint")}</small>
    </section>
  );
}
