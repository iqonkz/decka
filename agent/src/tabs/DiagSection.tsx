import { useState } from "react";
import { t, type DiagCheck, type TranslationKey } from "@decka/shared";
import { call } from "../ipc";
import Button from "../ui/Button";

/** The sentence of a check: `diag.<id>.<status>`; a failed firewall check has the kind of failure in front of its detail. */
export function checkText(c: DiagCheck): string {
  if (c.id === "firewall" && c.status === "fail") {
    const [kind = "", ...rest] = c.detail.split(":");
    return t(kind === "block" ? "diag.firewall.block" : "diag.firewall.missing", { detail: rest.join(":") });
  }
  const key = TEXT[`${c.id}.${c.status}`];
  return key === undefined ? `${c.id}: ${c.status} ${c.detail}`.trim() : t(key, { detail: c.detail });
}

const TEXT: Record<string, TranslationKey> = {
  "server.ok": "diag.server.ok",
  "server.fail": "diag.server.fail",
  "bind.ok": "diag.bind.ok",
  "bind.warn": "diag.bind.warn",
  "address.ok": "diag.address.ok",
  "address.fail": "diag.address.fail",
  "reach.ok": "diag.reach.ok",
  "reach.fail": "diag.reach.fail",
  "firewall.ok": "diag.firewall.ok",
  "firewall.unknown": "diag.firewall.unknown",
  "clients.ok": "diag.clients.ok",
  "clients.warn": "diag.clients.warn",
};

const MARK: Record<DiagCheck["status"], string> = { ok: "✓", warn: "!", fail: "✗", unknown: "?" };

/** `Check the connection`: runs the checks of the agent and shows one line each; a firewall problem gets a repair button. */
export default function DiagSection({ onFixed }: { onFixed?: () => void }) {
  const [checks, setChecks] = useState<DiagCheck[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [note, setNote] = useState<string | null>(null);

  const run = async () => {
    setBusy(true);
    setNote(null);
    try {
      setChecks(await call("diagnose_connection"));
    } catch (e) {
      setNote(String(e));
    } finally {
      setBusy(false);
    }
  };
  const fix = async () => {
    setBusy(true);
    try {
      await call("fix_firewall");
      setNote(t("diag.fix_done"));
      onFixed?.();
    } catch (e) {
      setNote(String(e));
    } finally {
      setBusy(false);
    }
  };
  const firewallBad = checks?.some((c) => c.id === "firewall" && c.status === "fail") === true;

  return (
    <section className="card" id="diag">
      <h3>{t("diag.title")}</h3>
      <p className="muted">{t("diag.intro")}</p>
      <div className="row">
        <Button onClick={() => void run()} disabled={busy}>
          {busy ? t("diag.running") : t("diag.run")}
        </Button>
        {firewallBad && (
          <Button onClick={() => void fix()} disabled={busy}>
            {t("diag.fix")}
          </Button>
        )}
      </div>
      {checks && (
        <ul className="diag-list">
          {checks.map((c) => (
            <li key={c.id} className={`diag-${c.status}`}>
              <span className="diag-mark" aria-hidden="true">{MARK[c.status]}</span> {checkText(c)}
            </li>
          ))}
        </ul>
      )}
      {checks && checks.some((c) => c.status === "fail" || c.id === "clients") && <p className="muted">{t("diag.same_network")}</p>}
      {note && <p className="muted">{note}</p>}
    </section>
  );
}
