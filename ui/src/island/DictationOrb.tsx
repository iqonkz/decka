import { t } from "@decka/shared";
import type { OrbState } from "./islandTypes";
import ParticlesOrb from "./ParticlesOrb";
import "./island.css";

export interface DictationOrbProps {
  state?: OrbState;
  level?: number;
  colorFrom?: string;
  colorTo?: string;
  theme?: "light" | "dark";
  /** Caret/cursor screen coordinates; renders +8 px right and +8 px below. */
  x?: number;
  y?: number;
  /** Short error bubble shown when there is no active input target (T29.4). */
  error?: string | null;
}

/**
 * Compact 24 px dictation orb shown near the active text caret, with an optional error bubble (TASKS.md §T29.4, §T29.10b).
 */
export default function DictationOrb({
  state = "listening",
  level = 0.5,
  colorFrom = "#22d3ee",
  colorTo = "#d946ef",
  theme = "dark",
  x,
  y,
  error,
}: DictationOrbProps) {
  const posStyle = x !== undefined && y !== undefined ? { left: x + 8, top: y + 8 } : undefined;
  return (
    <div className="island-host" data-island-theme={theme}>
      <div
        className={`island-dictation-wrap${posStyle ? " is-positioned" : ""}`}
        style={posStyle}
        role="status"
        aria-label={t("orb.aria")}
      >
        <div className={`island-orb-wrap dictation${error ? " is-error" : ""}`}>
          <ParticlesOrb
            state={error ? "idle" : state}
            size={24}
            particleCount={80}
            maxFps={24}
            level={level}
            colorFrom={colorFrom}
            colorTo={colorTo}
            staticFrame={Boolean(error)}
          />
        </div>
        {error && (
          <div className="island-dictation-bubble" role="alert">
            {error}
          </div>
        )}
      </div>
    </div>
  );
}
