import { useState } from "react";
import { t, type ConfigFile, type MailAccount } from "@decka/shared";
import { call } from "../ipc";
import { makeActionId } from "../editor/addButton";
import Banner from "../ui/Banner";
import Button from "../ui/Button";

const MAX_ACCOUNTS = 3;

/** The presets of the window (DATA_MODEL.md 10.7): the server, and the page a tap on the mailbox opens (Gmail only). */
export const MAIL_PRESETS = [
  { id: "gmail", name: "Gmail", host: "imap.gmail.com", url: "https://mail.google.com" },
  { id: "yandex", name: "Yandex", host: "imap.yandex.ru" },
  { id: "mailru", name: "Mail.ru", host: "imap.mail.ru" },
  { id: "icloud", name: "iCloud", host: "imap.mail.me.com" },
  { id: "other", name: "Other", host: "" },
] as const;

/** `More → Integrations → Mail` (DATA_MODEL.md 10.7): up to three mailboxes, IMAP over TLS, unread count only. */
export default function MailSection({ config, onSaved }: { config: ConfigFile; onSaved: () => Promise<void> }) {
  const accounts = config.mail?.accounts ?? [];
  const [error, setError] = useState<string | null>(null);
  const [passwordFor, setPasswordFor] = useState<string | null>(null);
  const [password, setPassword] = useState("");

  async function saveAccounts(next: MailAccount[]) {
    setError(null);
    const fresh = await call("get_config");
    // Only what the window may write: `passwordSet` is informational and the password itself never travels through here.
    const mail = { accounts: next.map(({ password: _password, ...rest }) => rest) };
    const result = await call("save_config", { config: { ...fresh, mail: next.length > 0 ? mail : undefined } });
    if (!result.ok) return setError(result.issues[0]?.message ?? "");
    await onSaved();
  }

  const change = (id: string, patch: Partial<MailAccount>) => saveAccounts(accounts.map((a) => (a.id === id ? { ...a, ...patch } : a)));

  async function add(presetId: string) {
    const preset = MAIL_PRESETS.find((p) => p.id === presetId) ?? MAIL_PRESETS[4];
    const id = makeActionId(preset.name, Object.fromEntries(accounts.map((a) => [a.id, 1])));
    const account: MailAccount = { id, name: preset.name, host: preset.host, port: 993, user: "" };
    // Gmail: a tap opens the web mail, through an ordinary `open_url` action that the window creates.
    if ("url" in preset) {
      const file = await call("get_actions");
      const actionId = makeActionId(`open-${id}`, file.actions);
      const saved = await call("save_actions", { actions: { ...file, actions: { ...file.actions, [actionId]: { name: `Open ${preset.name}`, type: "open_url", url: preset.url } } } });
      if (saved.ok) account.openActionId = actionId;
    }
    await saveAccounts([...accounts, account]);
  }

  async function savePassword(accountId: string) {
    setError(null);
    try {
      await call("set_mail_password", { accountId, password });
      setPassword("");
      setPasswordFor(null);
      await onSaved();
    } catch (e) {
      setError(String(e));
    }
  }

  return (
    <section className="card" id="mail">
      <h3>{t("mailset.title")}</h3>
      {accounts.map((a) => (
        <div className="mail-account" key={a.id}>
          <div className="row">
            <label className="field-row grow">
              <span>{t("mailset.name")}</span>
              <input defaultValue={a.name} maxLength={40} onBlur={(e) => e.target.value !== a.name && e.target.value.trim() !== "" && void change(a.id, { name: e.target.value.trim() })} />
            </label>
            <Button onClick={() => void saveAccounts(accounts.filter((x) => x.id !== a.id))}>{t("mailset.remove")}</Button>
          </div>
          <div className="row">
            <label className="field-row grow">
              <span>{t("mailset.host")}</span>
              <input defaultValue={a.host} onBlur={(e) => e.target.value !== a.host && void change(a.id, { host: e.target.value.trim() })} />
            </label>
            <label className="field-row">
              <span>{t("mailset.port")}</span>
              <input inputMode="numeric" defaultValue={String(a.port)} style={{ width: 80 }} onBlur={(e) => Number(e.target.value) !== a.port && void change(a.id, { port: Number(e.target.value) || 993 })} />
            </label>
          </div>
          <label className="field-row">
            <span>{t("mailset.user")}</span>
            <input defaultValue={a.user} autoComplete="off" onBlur={(e) => e.target.value !== a.user && void change(a.id, { user: e.target.value.trim() })} />
          </label>
          {passwordFor !== a.id ? (
            <div className="row">
              <Button onClick={() => setPasswordFor(a.id)}>{t("mailset.set_password")}</Button>
              <span className="muted">{a.passwordSet ? t("mailset.password_set") : t("mailset.no_password")}</span>
            </div>
          ) : (
            <div className="row">
              <input type="password" autoFocus autoComplete="off" className="grow" aria-label={t("mailset.password")} placeholder={t("mailset.password")} value={password} onChange={(e) => setPassword(e.target.value)} onKeyDown={(e) => e.key === "Enter" && void savePassword(a.id)} />
              <Button variant="primary" onClick={() => void savePassword(a.id)}>{password === "" ? t("mailset.remove_password") : t("common.save")}</Button>
              <Button onClick={() => { setPasswordFor(null); setPassword(""); }}>{t("common.cancel")}</Button>
            </div>
          )}
        </div>
      ))}
      {accounts.length < MAX_ACCOUNTS && (
        <div className="row">
          <select aria-label={t("mailset.add")} value="" onChange={(e) => e.target.value && void add(e.target.value)}>
            <option value="">{t("mailset.add")}</option>
            {MAIL_PRESETS.map((p) => (
              <option key={p.id} value={p.id}>{p.name}</option>
            ))}
          </select>
          {accounts.length > 0 && <Button onClick={() => void call("mail_check_now")}>{t("mailset.check_now")}</Button>}
        </div>
      )}
      {error && <Banner kind="error">{error}</Banner>}
      <small className="muted block">{t("mailset.hint")}</small>
    </section>
  );
}
