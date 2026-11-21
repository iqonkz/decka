import { useState } from "react";
import { sameMinor, t } from "@decka/shared";
import { startAppUpdate } from "../app/controller";
import { isNewerVersion } from "../update/apkAssembler";
import { hiddenStripApps, setStripMode, useStripModes } from "../deck/stripMode";
import { applyTurn } from "../platform/orientation";
import { isNative } from "../platform/native";
import { loadTurn, type Turn } from "../platform/turn";
import { loadVoiceButtonEnabled, saveVoiceButtonEnabled } from "../voice/voiceFabMath";
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
  const offered = useAppState((s) => s.state?.update?.app);
  const appUpdate = useAppState((s) => s.appUpdate);
  // A new app is offered only on the phone (the system installer), and only when it is newer than this one.
  const newApp = isNative() && offered && isNewerVersion(offered.version, import.meta.env.VITE_APP_VERSION) ? offered : null;
  const [turn, setTurn] = useState<Turn>(() => loadTurn());
  const [voiceButton, setVoiceButton] = useState(() => loadVoiceButtonEnabled());
  const hidden = hiddenStripApps(useStripModes());

  return (
    <div className="menu-backdrop" onPointerDown={onClose}>
      <div className="menu-sheet" role="dialog" aria-label={t("menu.label")} onPointerDown={(e) => e.stopPropagation()}>
        <div className="menu-status">{statusLine(conn, via, agentName)}</div>
        {newApp && (
          <button type="button" className="menu-row menu-button menu-update" disabled={appUpdate.phase === "downloading" || appUpdate.phase === "verifying"} onClick={() => void startAppUpdate()}>
            {appUpdate.phase === "downloading"
              ? t("update.downloading", { pct: appUpdate.pct })
              : appUpdate.phase === "verifying"
                ? t("update.app_verifying")
                : appUpdate.phase === "error"
                  ? appUpdate.kind === "bad"
                    ? t("update.app_bad")
                    : t("update.app_failed")
                  : t("update.app_banner", { version: newApp.version })}
          </button>
        )}
        <label className="menu-row">
          <span>{t("menu.reduce_effects")}</span>
          <input type="checkbox" checked={reduced} onChange={(e) => onToggleReduced(e.target.checked)} />
        </label>
        <label className="menu-row">
          <span>{t("menu.haptics")}</span>
          <input type="checkbox" checked={haptics} onChange={(e) => onToggleHaptics(e.target.checked)} />
        </label>
        <label className="menu-row">
          <span>{t("menu.voice_button")}</span>
          <input
            type="checkbox"
            checked={voiceButton}
            onChange={(e) => {
              const val = e.target.checked;
              setVoiceButton(val);
              saveVoiceButtonEnabled(val);
              window.dispatchEvent(new CustomEvent("decka:voice:enabled"));
            }}
          />
        </label>
        {isNative() && (
          <div className="menu-row menu-turn">
            <span>{t("menu.turn")}</span>
            <span className="menu-seg">
              {(["auto", "normal", "flipped"] as const).map((value) => (
                <button
                  key={value}
                  type="button"
                  className={turn === value ? "on" : ""}
                  onClick={() => {
                    setTurn(value);
                    applyTurn(value);
                  }}
                >
                  {t(`turn.${value}`)}
                </button>
              ))}
            </span>
          </div>
        )}
        {hidden.length > 0 && (
          <div className="menu-strips">
            <div className="menu-sub">{t("menu.app_strips")}</div>
            {hidden.map((app) => (
              <div key={app} className="menu-row">
                <span>{app}</span>
                <button type="button" className="menu-link" onClick={() => setStripMode(app, "auto")}>
                  {t("strip.show_again")}
                </button>
              </div>
            ))}
          </div>
        )}
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
