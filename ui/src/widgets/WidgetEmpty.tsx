import Icon from "../Icon";

/** Unified empty state for widgets (DESIGN.md 4.16): 24 px icon at 40 % + 12 px single line, no "—" rings/dashes. */
export default function WidgetEmpty({ icon, text, hint }: { icon: string; text: string; hint?: string }) {
  return (
    <div className="widget-empty">
      <span className="widget-empty-icon" aria-hidden="true">
        <Icon icon={icon} size={24} />
      </span>
      <span className="widget-empty-text">{text}</span>
      {hint && <span className="widget-empty-hint w-l">{hint}</span>}
    </div>
  );
}
