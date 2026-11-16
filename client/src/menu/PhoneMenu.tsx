import { sameMinor, t } from "@decka/shared";
import { useAppState } from "../store/store";
import { statusLine, versionLine } from "./menuModel";

interface Props {
  reduced: boolean;
  onToggleReduced: (reduced: boolean) => void;
  haptics: boolean;
  onToggleHaptics: (enabled: boolean) => void;
  onUnpair: () => void;
  onClose: () => void;
}

/** The phone menu (DESIGN.md 4.3): a sheet from the bottom with the connection, two switches, Reload and Unpair. */
export default function PhoneMenu({ reduced, onToggleReduced, haptics, onToggleHaptics, onUnpair, onClose }: Props) {
  const conn = useAppState((s) => s.conn);
  const via = useAppState((s) => s.via);
  const agentName = useAppState((s) => s.agentName);
  const agentVersion = useAppState((s) => s.agentVersion);

  return (
    <div className="menu-backdrop" onPointerDown={onClose}>
      <div className="menu-sheet" role="dialog" aria-label={t("menu.label")} onPointerDown={(e) => e.stopPropagation()}>
        <div className="menu-status">{statusLine(conn, via, agentName)}</div>
        <label className="menu-row">
          <span>{t("menu.reduce_effects")}</span>
          <input type="checkbox" checked={reduced} onChange={(e) => onToggleReduced(e.target.checked)} />
        </label>
        <label className="menu-row">
          <span>{t("menu.haptics")}</span>
          <input type="checkbox" checked={haptics} onChange={(e) => onToggleHaptics(e.target.checked)} />
        </label>
        <button type="button" className="menu-row menu-button" onClick={() => location.reload()}>
          {t("menu.reload")}
        </button>
        <button type="button" className="menu-row menu-button danger" onClick={onUnpair}>
          {t("menu.unpair")}
        </button>
        <div className="menu-version">{versionLine(import.meta.env.VITE_APP_VERSION, agentVersion)}</div>
        {agentVersion !== null && !sameMinor(import.meta.env.VITE_APP_VERSION, agentVersion) && (
          <div className="menu-version warn">{t("menu.pc_version", { version: agentVersion })}</div>
        )}
      </div>
    </div>
  );
}
