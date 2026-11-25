import { useState } from "react";
import { t, type ConfigFile, type MailAccount } from "@decka/shared";
import { call } from "../ipc";
import { makeActionId } from "../editor/addButton";
import Banner from "../ui/Banner";
import Button from "../ui/Button";
import Select from "../ui/Select";
import { showToast } from "../ui/Toast";

const MAX_ACCOUNTS = 3;

export const MAIL_PRESETS = [
  { id: "gmail", name: "Gmail", host: "imap.gmail.com", url: "https://mail.google.com" },
  { id: "yandex", name: "Yandex", host: "imap.yandex.ru" },
  { id: "mailru", name: "Mail.ru", host: "imap.mail.ru" },
  { id: "icloud", name: "iCloud", host: "imap.mail.me.com" },
  { id: "other", name: "Other", host: "" },
] as const;

interface AccountErrors {
  user?: string;
  host?: string;
  port?: string;
}

export default function MailSection({ config, onSaved }: { config: ConfigFile; onSaved: () => Promise<void> }) {
  const [accounts, setAccounts] = useState<MailAccount[]>(() => config.mail?.accounts ?? []);
  const [errors, setErrors] = useState<Record<string, AccountErrors>>({});
  const [passwordFor, setPasswordFor] = useState<string | null>(null);
  const [password, setPassword] = useState("");
  const [generalError, setGeneralError] = useState<string | null>(null);

  const updateAccountField = (id: string, patch: Partial<MailAccount>) => {
    setAccounts((prev) => prev.map((a) => (a.id === id ? { ...a, ...patch } : a)));
    // Clear field error on change
    if (errors[id]) {
      setErrors((prev) => ({
        ...prev,
        [id]: { ...prev[id], ...Object.fromEntries(Object.keys(patch).map((k) => [k, undefined])) },
      }));
    }
  };

  async function persistAccounts(next: MailAccount[]): Promise<boolean> {
    setGeneralError(null);
    try {
      const fresh = await call("get_config");
      const mail = { accounts: next.map(({ password: _password, ...rest }) => rest) };
      const result = await call("save_config", { config: { ...fresh, mail: next.length > 0 ? mail : undefined } });
      if (!result.ok) {
        const issue = result.issues[0];
        const msg = issue?.message ?? "";
        if (msg.includes("user")) {
          setGeneralError(t("mailset.err_user"));
        } else if (msg.includes("host")) {
          setGeneralError(t("mailset.err_host"));
        } else if (msg.includes("port")) {
          setGeneralError(t("mailset.err_port"));
        } else {
          setGeneralError(msg);
        }
        return false;
      }
      await onSaved();
      showToast(t("common.saved_toast"), "success");
      return true;
    } catch (e) {
      setGeneralError(String(e));
      return false;
    }
  }

  async function saveAccount(id: string) {
    const account = accounts.find((a) => a.id === id);
    if (!account) return;

    // Validate fields before submitting
    const accountErr: AccountErrors = {};
    if (!account.host.trim()) {
      accountErr.host = t("mailset.err_host");
    }
    if (!account.port || account.port < 1 || account.port > 65535) {
      accountErr.port = t("mailset.err_port");
    }
    if (!account.user.trim()) {
      accountErr.user = t("mailset.err_user");
    }

    if (Object.keys(accountErr).length > 0) {
      setErrors((prev) => ({ ...prev, [id]: accountErr }));
      return;
    }

    setErrors((prev) => ({ ...prev, [id]: {} }));
    await persistAccounts(accounts);
  }

  async function removeAccount(id: string) {
    const next = accounts.filter((x) => x.id !== id);
    setAccounts(next);
    setErrors((prev) => {
      const copy = { ...prev };
      delete copy[id];
      return copy;
    });
    await persistAccounts(next);
  }

  async function addPreset(presetId: string) {
    const preset = MAIL_PRESETS.find((p) => p.id === presetId) ?? MAIL_PRESETS[4];
    const id = makeActionId(preset.name, Object.fromEntries(accounts.map((a) => [a.id, 1])));
    const account: MailAccount = { id, name: preset.name, host: preset.host, port: 993, user: "" };

    if ("url" in preset) {
      try {
        const file = await call("get_actions");
        const actionId = makeActionId(`open-${id}`, file.actions);
        const saved = await call("save_actions", {
          actions: {
            ...file,
            actions: { ...file.actions, [actionId]: { name: `Open ${preset.name}`, type: "open_url", url: preset.url } },
          },
        });
        if (saved.ok) account.openActionId = actionId;
      } catch {}
    }

    // Add locally without saving incomplete account to backend yet
    setAccounts((prev) => [...prev, account]);
    setErrors((prev) => ({ ...prev, [id]: {} }));
  }

  async function savePassword(accountId: string) {
    setGeneralError(null);
    try {
      await call("set_mail_password", { accountId, password });
      setPassword("");
      setPasswordFor(null);
      await onSaved();
      showToast(t("common.saved_toast"), "success");
    } catch (e) {
      setGeneralError(String(e));
    }
  }

  return (
    <section className="card" id="mail">
      <h3>{t("mailset.title")}</h3>
      {accounts.map((a) => {
        const err = errors[a.id] ?? {};
        return (
          <div className="mail-account" key={a.id} style={{ display: "flex", flexDirection: "column", gap: 10, padding: "12px 0", borderBottom: "1px solid var(--border)" }}>
            <div className="row">
              <label className="field-row grow">
                <span>{t("mailset.name")}</span>
                <input
                  value={a.name}
                  maxLength={40}
                  onChange={(e) => updateAccountField(a.id, { name: e.target.value })}
                />
              </label>
              <Button onClick={() => void removeAccount(a.id)}>{t("mailset.remove")}</Button>
            </div>
            <div className="row">
              <label className="field-row grow">
                <span>{t("mailset.host")}</span>
                <input
                  value={a.host}
                  onChange={(e) => updateAccountField(a.id, { host: e.target.value })}
                />
                {err.host && <span className="field-error-text">{err.host}</span>}
              </label>
              <label className="field-row">
                <span>{t("mailset.port")}</span>
                <input
                  inputMode="numeric"
                  value={String(a.port)}
                  style={{ width: 80 }}
                  onChange={(e) => updateAccountField(a.id, { port: Number(e.target.value) || 0 })}
                />
                {err.port && <span className="field-error-text">{err.port}</span>}
              </label>
            </div>
            <label className="field-row">
              <span>{t("mailset.user")}</span>
              <input
                value={a.user}
                autoComplete="off"
                onChange={(e) => updateAccountField(a.id, { user: e.target.value })}
              />
              {err.user && <span className="field-error-text">{err.user}</span>}
            </label>

            <div className="row" style={{ justifyContent: "space-between", marginTop: 4 }}>
              {passwordFor !== a.id ? (
                <div className="row">
                  <Button onClick={() => setPasswordFor(a.id)}>{t("mailset.set_password")}</Button>
                  <span className="muted">{a.passwordSet ? t("mailset.password_set") : t("mailset.no_password")}</span>
                </div>
              ) : (
                <div className="row">
                  <input
                    type="password"
                    autoFocus
                    autoComplete="off"
                    className="grow"
                    aria-label={t("mailset.password")}
                    placeholder={t("mailset.password")}
                    value={password}
                    onChange={(e) => setPassword(e.target.value)}
                    onKeyDown={(e) => e.key === "Enter" && void savePassword(a.id)}
                  />
                  <Button variant="primary" onClick={() => void savePassword(a.id)}>
                    {password === "" ? t("mailset.remove_password") : t("common.save")}
                  </Button>
                  <Button onClick={() => { setPasswordFor(null); setPassword(""); }}>
                    {t("common.cancel")}
                  </Button>
                </div>
              )}

              <Button variant="primary" onClick={() => void saveAccount(a.id)}>
                {t("common.save")}
              </Button>
            </div>
          </div>
        );
      })}

      {accounts.length < MAX_ACCOUNTS && (
        <div className="row" style={{ marginTop: 12 }}>
          <Select
            aria-label={t("mailset.add")}
            value=""
            placeholder={t("mailset.add")}
            options={MAIL_PRESETS.map((p) => ({ value: p.id, label: p.name }))}
            onChange={(val) => val && void addPreset(val)}
          />
          {accounts.length > 0 && <Button onClick={() => void call("mail_check_now")}>{t("mailset.check_now")}</Button>}
        </div>
      )}
      {generalError && <Banner kind="error">{generalError}</Banner>}
      <small className="muted block" style={{ marginTop: 8 }}>{t("mailset.hint")}</small>
    </section>
  );
}
