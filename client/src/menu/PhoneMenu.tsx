import { useState } from "react";
import { sameMinor, t } from "@decka/shared";
import pkg from "../../package.json";
import { connectAgain, startAppUpdate } from "../app/controller";
import { isNewerVersion } from "../update/apkAssembler";
import { hiddenStripApps, setStripMode, useStripModes } from "../deck/stripMode";
import { applyTurn } from "../platform/orientation";
import { isNative } from "../platform/native";
import { loadTurn, type Turn } from "../platform/turn";
import { loadVoiceButtonEnabled, saveVoiceButtonEnabled } from "../voice/voiceFabMath";
import { useAppState } from "../store/store";
import PhIcon, { type PhIconName } from "../ui/PhIcon";
import PhoneSwitch from "../ui/PhoneSwitch";
import { statusLine, versionLine } from "./menuModel";

interface Props {
  reduced: boolean;
  onToggleReduced: (reduced: boolean) => void;
  haptics: boolean;
  onToggleHaptics: (enabled: boolean) => void;
  onUnpair: () => void;
  onClose: () => void;
}

/** The phone menu (DESIGN.md 4.3, 4.16): bottom sheet styled after the PC settings window (4.14). */
export default function PhoneMenu({ reduced, onToggleReduced, haptics, onToggleHaptics, onUnpair, onClose }: Props) {
  const conn = useAppState((s) => s.conn);
  const via = useAppState((s) => s.via);
  const agentName = useAppState((s) => s.agentName);
  const agentVersion = useAppState((s) => s.agentVersion);
  const offered = useAppState((s) => s.state?.update?.app);
  const appUpdate = useAppState((s) => s.appUpdate);
  const appVer = import.meta.env.VITE_APP_VERSION || pkg.version;
  // A new app is offered only on the phone (the system installer), and only when it is newer than this one.
  const newApp = isNative() && offered && isNewerVersion(offered.version, appVer) ? offered : null;
  const [turn, setTurn] = useState<Turn>(() => loadTurn());
  const [voiceButton, setVoiceButton] = useState(() => loadVoiceButtonEnabled());
  const hidden = hiddenStripApps(useStripModes());

  const statusIcon: PhIconName = conn !== "online" ? "wifi-slash" : via === "usb" ? "usb" : "wifi-high";

  return (
    <div className="menu-backdrop" onPointerDown={onClose}>
      <div className="menu-sheet" role="dialog" aria-label={t("menu.label")} onPointerDown={(e) => e.stopPropagation()}>
        <div className="sheet-handle" aria-hidden="true" />
        <div className="sheet-scroll">
          <div className="phone-group">
            <div className="phone-row menu-status-row">
              <PhIcon name={statusIcon} size={20} className="phone-row-icon" />
              <span className="phone-row-label">{statusLine(conn, via, agentName)}</span>
            </div>
            {newApp && (
              <button type="button" className="phone-row menu-button menu-update" disabled={appUpdate.phase === "downloading" || appUpdate.phase === "verifying"} onClick={() => void startAppUpdate()}>
                <PhIcon name="download-simple" size={20} className="phone-row-icon accent" />
                <span className="phone-row-label">
                  {appUpdate.phase === "downloading"
                    ? t("update.downloading", { pct: appUpdate.pct })
                    : appUpdate.phase === "verifying"
                      ? t("update.app_verifying")
                      : appUpdate.phase === "error"
                        ? appUpdate.kind === "bad"
                          ? t("update.app_bad")
                          : t("update.app_failed")
                        : t("update.app_banner", { version: newApp.version })}
                </span>
              </button>
            )}
          </div>

          <div className="phone-group">
            <div className="phone-row">
              <PhIcon name="sparkle" size={20} className="phone-row-icon" />
              <span className="phone-row-label">{t("menu.reduce_effects")}</span>
              <PhoneSwitch checked={reduced} onChange={onToggleReduced} label={t("menu.reduce_effects")} />
            </div>
            <div className="phone-row">
              <PhIcon name="vibrate" size={20} className="phone-row-icon" />
              <span className="phone-row-label">{t("menu.haptics")}</span>
              <PhoneSwitch checked={haptics} onChange={onToggleHaptics} label={t("menu.haptics")} />
            </div>
            <div className="phone-row">
              <PhIcon name="microphone" size={20} className="phone-row-icon" />
              <span className="phone-row-label">{t("menu.voice_button")}</span>
              <PhoneSwitch
                checked={voiceButton}
                label={t("menu.voice_button")}
                onChange={(val) => {
                  setVoiceButton(val);
                  saveVoiceButtonEnabled(val);
                  window.dispatchEvent(new CustomEvent("decka:voice:enabled"));
                }}
              />
            </div>
            {isNative() && (
              <div className="phone-row phone-row-wrap">
                <PhIcon name="device-mobile" size={20} className="phone-row-icon" />
                <span className="phone-row-label">{t("menu.turn")}</span>
                <span className="phone-seg">
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
          </div>

          {hidden.length > 0 && (
            <>
              <div className="phone-group-title">{t("menu.app_strips")}</div>
              <div className="phone-group">
                {hidden.map((app) => (
                  <div key={app} className="phone-row">
                    <PhIcon name="squares-four" size={20} className="phone-row-icon" />
                    <span className="phone-row-label">{app}</span>
                    <button type="button" className="menu-link" onClick={() => setStripMode(app, "auto")}>
                      {t("strip.show_again")}
                    </button>
                  </div>
                ))}
              </div>
            </>
          )}

          <div className="phone-group">
            <button type="button" className="phone-row menu-button" onClick={() => location.reload()}>
              <PhIcon name="arrows-clockwise" size={20} className="phone-row-icon" />
              <span className="phone-row-label">{t("menu.reload")}</span>
            </button>
            <button type="button" className="phone-row menu-button" onClick={connectAgain}>
              <PhIcon name="qr-code" size={20} className="phone-row-icon" />
              <span className="phone-row-label">{t("link.connect_again")}</span>
            </button>
            <button type="button" className="phone-row menu-button danger" onClick={onUnpair}>
              <PhIcon name="link-break" size={20} className="phone-row-icon" />
              <span className="phone-row-label">{t("menu.unpair")}</span>
            </button>
          </div>

          <div className="menu-version">{versionLine(appVer, agentVersion)}</div>
          {agentVersion !== null && !sameMinor(appVer, agentVersion) && (
            <div className="menu-version warn">{t("menu.pc_version", { version: agentVersion })}</div>
          )}
        </div>
      </div>
    </div>
  );
}
