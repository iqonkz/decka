import { t } from "@decka/shared";

/**
 * QR scanner viewfinder overlay (DESIGN.md 4.16): 4 px accent corner brackets with dimmed surround.
 * Note: real Android native barcode scanning opens the Capacitor scanner plugin activity.
 */
export default function QrFrame({ onClose }: { onClose?: () => void }) {
  return (
    <div className="qr-overlay" role="dialog" aria-label={t("pair.scan")}>
      <div className="qr-box">
        <span className="qr-corner tl" aria-hidden="true" />
        <span className="qr-corner tr" aria-hidden="true" />
        <span className="qr-corner bl" aria-hidden="true" />
        <span className="qr-corner br" aria-hidden="true" />
      </div>
      <p className="qr-hint">{t("pair.qr_hint")}</p>
      {onClose && (
        <button type="button" className="pill-btn secondary qr-close" onClick={onClose}>
          {t("common.close")}
        </button>
      )}
    </div>
  );
}
