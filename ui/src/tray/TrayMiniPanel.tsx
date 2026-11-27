import { t } from "@decka/shared";
import { Mic, MicOff, Music, Pause, Play, Settings, SkipBack, SkipForward, Video } from "lucide-react";
import Icon from "../Icon";
import "./tray.css";

export interface TrayMiniAction {
  id: string;
  label: string;
  icon: string;
  active?: boolean;
}

export interface TrayMiniNextEvent {
  title: string;
  startTime: string;
  joinActionId?: string;
}

export interface TrayMiniPanelProps {
  theme?: "light" | "dark";
  track?: {
    title: string;
    artist?: string;
    art?: string;
    playing: boolean;
  } | null;
  volume?: number;
  muted?: boolean;
  micMuted?: boolean;
  favorites: TrayMiniAction[];
  nextEvent?: TrayMiniNextEvent | null;
  onMedia?: (op: "prev" | "play_pause" | "next") => void;
  onVolumeChange?: (pct: number) => void;
  onToggleMute?: () => void;
  onToggleMic?: () => void;
  onFavorite?: (actionId: string) => void;
  onJoinEvent?: (actionId: string) => void;
  onOpenSettings?: () => void;
}

/**
 * 320x420 Windows tray mini-panel (M32 T32.1b / T32.3): media player, volume + mic,
 * 6 favorite actions, upcoming calendar event, and Open settings button.
 */
export default function TrayMiniPanel({
  theme = "dark",
  track,
  volume = 42,
  muted = false,
  micMuted = false,
  favorites,
  nextEvent,
  onMedia,
  onVolumeChange,
  onToggleMute,
  onToggleMic,
  onFavorite,
  onJoinEvent,
  onOpenSettings,
}: TrayMiniPanelProps) {
  const pct = Math.min(100, Math.max(0, Math.round(volume)));
  const shownFavs = favorites.slice(0, 6);

  return (
    <div className="tray-mini" data-tray-theme={theme}>
      {/* Player */}
      <div className="tray-card tray-player">
        {track?.art ? (
          <img className="tray-art" src={track.art} alt="" width={40} height={40} />
        ) : (
          <div className="tray-art tray-art-empty" aria-hidden="true">
            <Music size={18} strokeWidth={1.75} />
          </div>
        )}
        <div className="tray-track-copy">
          <div className="tray-track-title">{track?.title ?? "—"}</div>
          {track?.artist && <div className="tray-track-artist">{track.artist}</div>}
        </div>
        <div className="tray-media-btns">
          <button type="button" className="tray-icon-btn" aria-label={t("preset.prev")} onClick={() => onMedia?.("prev")}>
            <SkipBack size={15} strokeWidth={1.75} />
          </button>
          <button type="button" className="tray-icon-btn primary" aria-label={t("preset.play_pause")} onClick={() => onMedia?.("play_pause")}>
            {track?.playing ? <Pause size={15} strokeWidth={1.75} /> : <Play size={15} strokeWidth={1.75} />}
          </button>
          <button type="button" className="tray-icon-btn" aria-label={t("preset.next")} onClick={() => onMedia?.("next")}>
            <SkipForward size={15} strokeWidth={1.75} />
          </button>
        </div>
      </div>

      {/* Volume + Mic */}
      <div className="tray-card tray-audio-row">
        <button
          type="button"
          className="tray-vol-track"
          aria-label={t("widget_name.volume")}
          onClick={() => onToggleMute?.()}
        >
          <span className="tray-vol-fill" style={{ width: `${muted ? 0 : pct}%` }} />
          <span className="tray-vol-label">{t("widget_name.volume")}</span>
          <span className="tray-vol-pct">{pct}%</span>
        </button>
        {onVolumeChange && (
          <input
            className="tray-vol-range"
            type="range"
            min={0}
            max={100}
            value={pct}
            aria-label={t("widget_name.volume")}
            onChange={(e) => onVolumeChange(Number(e.target.value))}
          />
        )}
        <button
          type="button"
          className={`tray-mic-btn${micMuted ? " is-muted" : " is-live"}`}
          onClick={() => onToggleMic?.()}
        >
          {micMuted ? <MicOff size={15} strokeWidth={1.75} /> : <Mic size={15} strokeWidth={1.75} />}
          <span>{micMuted ? t("m32.tray_mic_off") : t("m32.tray_mic_on")}</span>
        </button>
      </div>

      {/* 6 Favorite Actions */}
      <div className="tray-card tray-favs-card">
        <div className="tray-sec-title">{t("m32.tray_favorites")}</div>
        <div className="tray-favs-grid">
          {shownFavs.map((act) => (
            <button
              key={act.id}
              type="button"
              className={`tray-fav-btn${act.active ? " active" : ""}`}
              onClick={() => onFavorite?.(act.id)}
            >
              <Icon icon={act.icon} size={16} />
              <span className="tray-fav-label">{act.label}</span>
            </button>
          ))}
        </div>
      </div>

      {/* Calendar */}
      <div className="tray-card tray-cal-card">
        {nextEvent ? (
          <>
            <div className="tray-cal-meta">
              <span className="tray-cal-time">{nextEvent.startTime}</span>
              <span className="tray-cal-title">{nextEvent.title}</span>
            </div>
            {nextEvent.joinActionId && (
              <button type="button" className="tray-join-btn" onClick={() => onJoinEvent?.(nextEvent.joinActionId!)}>
                <Video size={13} strokeWidth={1.75} />
                <span>{t("m32.cal_join")}</span>
              </button>
            )}
          </>
        ) : (
          <span className="tray-cal-empty">{t("m32.cal_empty")}</span>
        )}
      </div>

      {/* Footer */}
      <button type="button" className="tray-footer-btn" onClick={() => onOpenSettings?.()}>
        <Settings size={15} strokeWidth={1.75} />
        <span>{t("m32.tray_open_settings")}</span>
      </button>
    </div>
  );
}
