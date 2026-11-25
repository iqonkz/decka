import { useEffect, useState } from "react";
import { t, type ConfigFile, type TwitchStatus } from "@decka/shared";
import { call } from "../ipc";
import Banner from "../ui/Banner";
import Button from "../ui/Button";
import Switch from "../ui/Switch";

/** `More → Integrations → Twitch`: the client id of your own Twitch application, the sign-in by a code, and the state. */
export default function TwitchSection({ config, onSaved }: { config: ConfigFile; onSaved: () => Promise<void> }) {
  const saved = config.twitch;
  const [enabled, setEnabled] = useState(saved?.enabled ?? false);
  const [clientId, setClientId] = useState(saved?.clientId ?? "");
  const [status, setStatus] = useState<TwitchStatus | null>(null);
  const [error, setError] = useState<string | null>(null);

  // What the agent sees is asked for every 2 seconds (the sign-in finishes in the background).
  useEffect(() => {
    let live = true;
    const ask = () => void call("twitch_status").then((s) => live && setStatus(s), () => live && setStatus(null));
    ask();
    const timer = window.setInterval(ask, 2000);
    return () => {
      live = false;
      window.clearInterval(timer);
    };
  }, []);
  // The sign-in finished: the agent turned the switch on, so the form follows.
  useEffect(() => {
    if (status?.signedIn) void onSaved();
  }, [status?.signedIn]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => {
    setEnabled(saved?.enabled ?? false);
  }, [saved?.enabled]);

  async function save(next: { enabled: boolean; clientId: string }) {
    setError(null);
    const fresh = await call("get_config");
    const result = await call("save_config", { config: { ...fresh, twitch: { enabled: next.enabled, clientId: next.clientId.trim() } } });
    if (!result.ok) return setError(result.issues[0]?.message ?? "");
    await onSaved();
  }

  async function signIn() {
    setError(null);
    try {
      await save({ enabled, clientId }); // the agent reads the client id from the saved settings
      await call("twitch_login_start");
      setStatus(await call("twitch_status"));
    } catch (e) {
      setError(String(e));
    }
  }

  async function signOut() {
    setError(null);
    try {
      await call("twitch_logout");
      setStatus(await call("twitch_status"));
      await onSaved();
    } catch (e) {
      setError(String(e));
    }
  }

  const pending = status?.pending;
  return (
    <section className="card" id="twitch">
      <h3>{t("twitchset.title")}</h3>
      <label className="switch-row">
        <Switch
          checked={enabled}
          label={t("twitchset.enable")}
          onChange={(checked) => {
            setEnabled(checked);
            void save({ enabled: checked, clientId });
          }}
        />
        <span>{t("twitchset.enable")}</span>
      </label>
      <label className="field-row">
        <span>{t("twitchset.client_id")}</span>
        <input className="mono grow" autoComplete="off" spellCheck={false} maxLength={64} value={clientId} onChange={(e) => setClientId(e.target.value.replace(/[^A-Za-z0-9]/g, ""))} onBlur={() => void save({ enabled, clientId })} />
      </label>
      <small className="muted block">{t("twitchset.hint")}</small>

      <div className="row">
        <strong className={status?.signedIn ? "ok-text" : "muted"}>{status?.signedIn ? t("twitchset.signed_in", { login: status.login ?? "" }) : t("twitchset.not_signed_in")}</strong>
        <span className="grow" />
        {status?.signedIn ? (
          <Button onClick={() => void signOut()}>{t("twitchset.sign_out")}</Button>
        ) : (
          <Button variant="primary" disabled={clientId.trim() === ""} onClick={() => void signIn()}>
            {t("twitchset.sign_in")}
          </Button>
        )}
      </div>
      {pending && !status?.signedIn && (
        <Banner kind="info">
          <span>{t("twitchset.type_code", { uri: pending.verificationUri.split("?")[0] ?? pending.verificationUri })}</span>
          <strong className="mono">{pending.userCode}</strong>
          <span className="muted">{t("twitchset.waiting")}</span>
        </Banner>
      )}
      {(error ?? status?.error) && <Banner kind="error">{error ?? status?.error}</Banner>}
      <small className="muted block">{t("twitchset.scopes")}</small>
    </section>
  );
}
