import { useState, type FormEvent } from "react";
import { normalizeCode, parseHostInput, parsePairUrl, t } from "@decka/shared";
import { cancelConnectAgain, pairErrorText, pairWith } from "../app/controller";
import { isNative } from "../platform/native";
import { scanQr } from "../platform/scan";
import { useAppState } from "../store/store";
import type { ConnFailure } from "../net/connection";
import ServiceCard from "../ui/ServiceCard";
import QrFrame from "./QrFrame";

export default function PairScreen({ showQrFrame = false }: { showQrFrame?: boolean }) {
  const native = isNative();
  const initialError = useAppState((s) => s.lastError);
  const conn = useAppState((s) => s.conn);
  // In the browser the page is served by the PC itself, so the address is known and hidden.
  const [address, setAddress] = useState(native ? "" : location.host);
  const [code, setCode] = useState("");
  const [error, setError] = useState<string | null>(initialError);
  const [busy, setBusy] = useState(false);
  const [qrOpen, setQrOpen] = useState(showQrFrame);

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
    <>
      <ServiceCard icon="qr-code" title={t("pair.title")} text={t("pair.hint")} asForm onSubmit={submit}>
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
        <div className="service-actions">
          {native ? (
            <>
              <button className="pill-btn primary" type="button" disabled={busy} onClick={() => void scan()}>
                {t("pair.scan")}
              </button>
              <button className="pill-btn secondary" type="submit" disabled={busy}>
                {busy ? t("pair.busy") : t("pair.submit")}
              </button>
            </>
          ) : (
            <button className="pill-btn primary" type="submit" disabled={busy}>
              {busy ? t("pair.busy") : t("pair.submit")}
            </button>
          )}
          {conn === "repair" && (
            <button className="pill-btn secondary" type="button" disabled={busy} onClick={cancelConnectAgain}>
              {t("link.cancel")}
            </button>
          )}
        </div>
      </ServiceCard>
      {qrOpen && <QrFrame onClose={() => setQrOpen(false)} />}
    </>
  );
}
