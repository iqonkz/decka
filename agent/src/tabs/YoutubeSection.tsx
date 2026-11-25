import { useEffect, useState } from "react";
import { t, type ConfigFile, type YoutubeStatus } from "@decka/shared";
import { call } from "../ipc";
import Banner from "../ui/Banner";
import Button from "../ui/Button";
import Switch from "../ui/Switch";

/** `More → Integrations → YouTube`: the client id and secret of your own Google application, the sign-in by a code, and the state. */
export default function YoutubeSection({ config, onSaved }: { config: ConfigFile; onSaved: () => Promise<void> }) {
  const saved = config.youtube;
  const [enabled, setEnabled] = useState(saved?.enabled ?? false);
  const [clientId, setClientId] = useState(saved?.clientId ?? "");
  const [editingSecret, setEditingSecret] = useState(false);
  const [secret, setSecret] = useState("");
  const [status, setStatus] = useState<YoutubeStatus | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    const ask = () => void call("youtube_status").then((s) => live && setStatus(s), () => live && setStatus(null));
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
    const result = await call("save_config", { config: { ...fresh, youtube: { enabled: next.enabled, clientId: next.clientId.trim() } } });
    if (!result.ok) return setError(result.issues[0]?.message ?? "");
    await onSaved();
  }

  async function saveSecret() {
    setError(null);
    try {
      await call("set_youtube_secret", { secret });
      setSecret("");
      setEditingSecret(false);
      setStatus(await call("youtube_status"));
      await onSaved();
    } catch (e) {
      setError(String(e));
    }
  }

  async function signIn() {
    setError(null);
    try {
      await save({ enabled, clientId });
      await call("youtube_login_start");
      setStatus(await call("youtube_status"));
    } catch (e) {
      setError(String(e));
    }
  }

  async function signOut() {
    setError(null);
    try {
      await call("youtube_logout");
      setStatus(await call("youtube_status"));
      await onSaved();
    } catch (e) {
      setError(String(e));
    }
  }

  const pending = status?.pending;
  const secretSet = status?.secretSet === true;
  return (
    <section className="card" id="youtube">
      <h3>{t("youtubeset.title")}</h3>
      <label className="switch-row">
        <Switch
          checked={enabled}
          label={t("youtubeset.enable")}
          onChange={(checked) => {
            setEnabled(checked);
            void save({ enabled: checked, clientId });
          }}
        />
        <span>{t("youtubeset.enable")}</span>
      </label>
      <label className="field-row">
        <span>{t("youtubeset.client_id")}</span>
        <input className="mono grow" autoComplete="off" spellCheck={false} maxLength={200} value={clientId} onChange={(e) => setClientId(e.target.value.replace(/[^A-Za-z0-9._-]/g, ""))} onBlur={() => void save({ enabled, clientId })} />
      </label>
      {!editingSecret ? (
        <div className="row">
          <Button onClick={() => setEditingSecret(true)}>{t("youtubeset.set_secret")}</Button>
          <span className="muted">{secretSet ? t("youtubeset.secret_set") : t("youtubeset.no_secret")}</span>
        </div>
      ) : (
        <div className="row">
          <input type="password" autoFocus autoComplete="off" className="grow" aria-label={t("youtubeset.secret")} placeholder={t("youtubeset.secret")} value={secret} onChange={(e) => setSecret(e.target.value)} onKeyDown={(e) => e.key === "Enter" && void saveSecret()} />
          <Button variant="primary" onClick={() => void saveSecret()}>{secret === "" ? t("youtubeset.remove") : t("common.save")}</Button>
          <Button onClick={() => { setEditingSecret(false); setSecret(""); }}>{t("common.cancel")}</Button>
        </div>
      )}
      <small className="muted block">{t("youtubeset.hint")}</small>

      <div className="row">
        <strong className={status?.signedIn ? "ok-text" : "muted"}>{status?.signedIn ? t("youtubeset.signed_in", { channel: status.channel ?? "" }) : t("youtubeset.not_signed_in")}</strong>
        <span className="grow" />
        {status?.signedIn ? (
          <Button onClick={() => void signOut()}>{t("youtubeset.sign_out")}</Button>
        ) : (
          <Button variant="primary" disabled={clientId.trim() === "" || !secretSet} onClick={() => void signIn()}>
            {t("youtubeset.sign_in")}
          </Button>
        )}
      </div>
      {pending && !status?.signedIn && (
        <Banner kind="info">
          <span>{t("youtubeset.type_code", { uri: pending.verificationUri })}</span>
          <strong className="mono">{pending.userCode}</strong>
          <span className="muted">{t("youtubeset.waiting")}</span>
        </Banner>
      )}
      {(error ?? status?.error) && <Banner kind="error">{error ?? status?.error}</Banner>}
      <small className="muted block">{t("youtubeset.quota")}</small>
    </section>
  );
}
