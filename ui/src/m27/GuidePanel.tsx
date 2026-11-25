import { t } from "@decka/shared";
import type { GuideStepEvent } from "./m27Types";
import "./m27.css";

export interface GuidePanelProps {
  guide: GuideStepEvent | null;
  theme?: "light" | "dark";
  accent?: string;
  onBack?: () => void;
  onStop?: () => void;
}

/**
 * Step-by-step Guide section rendered inside the Island surface (TASKS.md §T27.5 / §T27.2b):
 * shows the goal, "Step N" badge, current step text, Back, and Stop buttons.
 */
export default function GuidePanel({ guide, theme = "dark", accent = "#7c5cff", onBack, onStop }: GuidePanelProps) {
  if (!guide || !guide.active) return null;

  const stepLabel =
    guide.total && guide.total > 0 ? t("m27.step_of", { n: guide.step, total: guide.total }) : t("m27.step", { n: guide.step });

  return (
    <div className="m27-guide-card" data-m27-theme={theme} style={{ "--m27-accent": accent } as React.CSSProperties}>
      <div className="m27-guide-head">
        <span className="m27-guide-goal">{guide.goal}</span>
        <span className="m27-step-badge">{stepLabel}</span>
      </div>
      <div className="m27-guide-text">{guide.text}</div>
      {guide.etaSec ? <div className="m27-guide-eta">{t("island.slow_eta", { s: guide.etaSec })}</div> : null}
      <div className="m27-guide-actions">
        <button type="button" className="m27-guide-btn" disabled={guide.step <= 1} onClick={onBack}>
          {t("m27.back")}
        </button>
        <button type="button" className="m27-guide-btn is-stop" onClick={onStop}>
          {t("m27.stop")}
        </button>
      </div>
    </div>
  );
}
