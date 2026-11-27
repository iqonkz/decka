import { useState } from "react";
import { sameMinor, t } from "@decka/shared";
import { Icon } from "@decka/ui";
import pkg from "../../package.json";
import { connectAgain, startAppUpdate } from "../app/controller";
import { isNewerVersion } from "../update/apkAssembler";
import { hiddenStripApps, setStripMode, useStripModes } from "../deck/stripMode";
import { applyTurn } from "../platform/orientation";
import { isNative } from "../platform/native";
import { loadTurn, type Turn } from "../platform/turn";
import { loadVoiceButtonEnabled, saveVoiceButtonEnabled } from "../voice/voiceFabMath";
import { useAppState } from "../store/store";
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

/** The phone menu (DESIGN.md 4.3, M31 T31.8b): 56 px rows, PC-matching sections, 20 px Lucide icons. */
export default function PhoneMenu({ reduced, onToggleReduced, haptics, onToggleHaptics, onUnpair, onClose }: Props) {
  const conn = useAppState((s) => s.conn);
  const via = useAppState((s) => s.via);
  const agentName = useAppState((s) => s.agentName);
  const agentVersion = useAppState((s) => s.agentVersion);
  const offered = useAppState((s) => s.state?.update?.app);
  const appUpdate = useAppState((s) => s.appUpdate);
  const appVer = import.meta.env.VITE_APP_VERSION || pkg.version;
  const newApp = isNative() && offered && isNewerVersion(offered.version, appVer) ? offered : null;
  const [turn, setTurn] = useState<Turn>(() => loadTurn());
  const [voiceButton, setVoiceButton] = useState(() => loadVoiceButtonEnabled());
  const hidden = hiddenStripApps(useStripModes());

  const statusIcon = conn !== "online" ? "lucide:wifi-off" : via === "usb" ? "lucide:usb" : "lucide:wifi";

  return (
    <div className="menu-backdrop" onPointerDown={onClose}>
      <div className="menu-sheet" role="dialog" aria-label={t("menu.label")} onPointerDown={(e) => e.stopPropagation()}>
        <div className="sheet-handle" aria-hidden="true" />
        <div className="sheet-scroll">
          <div className="phone-group-title">{t("sidebar.group.connections")}</div>
          <div className="phone-group">
            <div className="phone-row menu-status-row">
              <span className="phone-row-icon"><Icon icon={statusIcon} size={20} /></span>
              <span className="phone-row-label">{statusLine(conn, via, agentName)}</span>
            </div>
            {newApp && (
              <button
                type="button"
                className="phone-row menu-button menu-update"
                disabled={appUpdate.phase === "downloading" || appUpdate.phase === "verifying"}
                onClick={() => void startAppUpdate()}
              >
                <span className="phone-row-icon accent"><Icon icon="lucide:download" size={20} /></span>
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

          <div className="phone-group-title">{t("sidebar.item.settings")}</div>
          <div className="phone-group">
            <div className="phone-row">
              <span className="phone-row-icon"><Icon icon="lucide:zap" size={20} /></span>
              <span className="phone-row-label">{t("menu.reduce_effects")}</span>
              <PhoneSwitch checked={reduced} onChange={onToggleReduced} label={t("menu.reduce_effects")} />
            </div>
            <div className="phone-row">
              <span className="phone-row-icon"><Icon icon="lucide:activity" size={20} /></span>
              <span className="phone-row-label">{t("menu.haptics")}</span>
              <PhoneSwitch checked={haptics} onChange={onToggleHaptics} label={t("menu.haptics")} />
            </div>
            <div className="phone-row">
              <span className="phone-row-icon"><Icon icon="lucide:mic" size={20} /></span>
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
                <span className="phone-row-icon"><Icon icon="lucide:smartphone" size={20} /></span>
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
                    <span className="phone-row-icon"><Icon icon="lucide:layout-grid" size={20} /></span>
                    <span className="phone-row-label">{app}</span>
                    <button type="button" className="menu-link" onClick={() => setStripMode(app, "auto")}>
                      {t("strip.show_again")}
                    </button>
                  </div>
                ))}
              </div>
            </>
          )}

          <div className="phone-group-title">{t("sidebar.group.app")}</div>
          <div className="phone-group">
            <button type="button" className="phone-row menu-button" onClick={() => location.reload()}>
              <span className="phone-row-icon"><Icon icon="lucide:refresh-cw" size={20} /></span>
              <span className="phone-row-label">{t("menu.reload")}</span>
            </button>
            <button type="button" className="phone-row menu-button" onClick={connectAgain}>
              <span className="phone-row-icon"><Icon icon="lucide:camera" size={20} /></span>
              <span className="phone-row-label">{t("link.connect_again")}</span>
            </button>
            <button type="button" className="phone-row menu-button danger" onClick={onUnpair}>
              <span className="phone-row-icon"><Icon icon="lucide:log-out" size={20} /></span>
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
