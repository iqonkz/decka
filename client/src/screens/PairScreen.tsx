import { useState, type FormEvent } from "react";
import { normalizeCode, parseHostInput, parsePairUrl, t } from "@decka/shared";
import { cancelConnectAgain, pairErrorText, pairWith } from "../app/controller";
import { isNative } from "../platform/native";
import { scanQr } from "../platform/scan";
import { useAppState } from "../store/store";
import type { ConnFailure } from "../net/connection";
import PhIcon from "../ui/PhIcon";
import QrFrame from "./QrFrame";

/**
 * Full-screen QR scanner pairing screen (M31 T31.8b): viewfinder frame in the upper stage,
 * manual pairing code entry sheet docked at the bottom, and human-readable word errors.
 */
export default function PairScreen({ showQrFrame = false }: { showQrFrame?: boolean }) {
  const native = isNative();
  const initialError = useAppState((s) => s.lastError);
  const conn = useAppState((s) => s.conn);
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
    if (text === null) return;
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
      <div className="pair-fullscreen" role="region" aria-label={t("pair.title")}>
        <div className="pair-scanner-stage">
          <div className="pair-scanner-head">
            <h1 className="service-title">{t("pair.title")}</h1>
            {native && <p className="service-text">{t("pair.qr_hint")}</p>}
          </div>
          {/* The frame opens the scanner of the app; a browser has no scanner, so it shows no frame. */}
          {native && (
            <div className="qr-box pair-viewfinder" onClick={() => void scan()}>
              <span className="qr-corner tl" aria-hidden="true" />
              <span className="qr-corner tr" aria-hidden="true" />
              <span className="qr-corner bl" aria-hidden="true" />
              <span className="qr-corner br" aria-hidden="true" />
              <span className="pair-viewfinder-icon" aria-hidden="true">
                <PhIcon name="qr-code" size={44} />
              </span>
            </div>
          )}
          {native && (
            <button className="pill-btn primary pair-scan-btn" type="button" disabled={busy} onClick={() => void scan()}>
              {t("pair.scan")}
            </button>
          )}
        </div>

        <form className="pair-bottom-sheet" onSubmit={submit}>
          <p className="pair-bottom-hint">{t("pair.hint")}</p>
          {error && (
            <div className="pair-error-banner" role="alert">
              <PhIcon name="warning-circle" size={16} />
              <span>{error}</span>
            </div>
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
          <div className="pair-code-row">
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
            <button className="pill-btn primary pair-submit-btn" type="submit" disabled={busy}>
              {busy ? t("pair.busy") : t("pair.submit")}
            </button>
          </div>
          {conn === "repair" && (
            <button className="pill-btn secondary" type="button" disabled={busy} onClick={cancelConnectAgain}>
              {t("link.cancel")}
            </button>
          )}
        </form>
      </div>
      {qrOpen && <QrFrame onClose={() => setQrOpen(false)} />}
    </>
  );
}
