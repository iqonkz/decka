import { useState, type FormEvent } from "react";
import logoUrl from "../../../docs/assets/decka-logo.svg";
import { normalizeCode, parseHostInput, parsePairUrl, t } from "@decka/shared";
import { cancelConnectAgain, pairErrorText, pairWith } from "../app/controller";
import { isNative } from "../platform/native";
import { scanQr } from "../platform/scan";
import { useAppState } from "../store/store";
import type { ConnFailure } from "../net/connection";

export default function PairScreen() {
  const native = isNative();
  const initialError = useAppState((s) => s.lastError);
  const conn = useAppState((s) => s.conn);
  // In the browser the page is served by the PC itself, so the address is known and hidden.
  const [address, setAddress] = useState(native ? "" : location.host);
  const [code, setCode] = useState("");
  const [error, setError] = useState<string | null>(initialError);
  const [busy, setBusy] = useState(false);

  async function submit(e: FormEvent) {
    e.preventDefault();
    const host = parseHostInput(address);
    if (!host) return setError(t("pair.error.address"));
    const normalized = normalizeCode(code);
    if (!normalized) return setError(t("pair.error.code"));
    setError(null);
    setBusy(true);
    try {
      await pairWith([host.host], host.port, normalized, host.host);
    } catch (err) {
      setError(pairErrorText((err as ConnFailure).code));
      setBusy(false);
    }
  }

  async function scan() {
    setError(null);
    const text = await scanQr();
    if (text === null) return; // cancelled: stay on the screen
    const parsed = parsePairUrl(text);
    if (!parsed) return setError(t("pair.error.not_qr"));
    setBusy(true);
    try {
      await pairWith(parsed.hosts, parsed.port, parsed.code, parsed.agentName, parsed.secret);
    } catch (err) {
      setError(pairErrorText((err as ConnFailure).code));
      setBusy(false);
    }
  }

  return (
    <form className="pair" onSubmit={submit}>
      <img src={logoUrl} width={72} height={72} alt="" />
      <h1>{t("pair.title")}</h1>
      <p className="hint">{t("pair.hint")}</p>
      {native && (
        <button className="primary" type="button" disabled={busy} onClick={() => void scan()}>
          {t("pair.scan")}
        </button>
      )}
      {native && (
        <input
          className="field"
          aria-label={t("pair.address_label")}
          placeholder="192.168.1.10"
          inputMode="url"
          autoCapitalize="off"
          value={address}
          onChange={(e) => setAddress(e.target.value)}
        />
      )}
      <input
        className="field code"
        aria-label={t("pair.code_label")}
        placeholder="XXXX-XXXX"
        autoCapitalize="characters"
        autoComplete="off"
        maxLength={9}
        value={code}
        onChange={(e) => setCode(e.target.value.toUpperCase())}
      />
      {error && <p className="error">{error}</p>}
      <button className="primary" type="submit" disabled={busy}>
        {busy ? t("pair.busy") : t("pair.submit")}
      </button>
      {conn === "repair" && (
        <button type="button" disabled={busy} onClick={cancelConnectAgain}>
          {t("link.cancel")}
        </button>
      )}
    </form>
  );
}
