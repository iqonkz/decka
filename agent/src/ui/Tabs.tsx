import PhIcon from "./PhIcon";

export interface TabDef<Id extends string> {
  id: Id;
  label: string;
  /** Name of a Phosphor icon for the side menu. */
  icon?: string;
  /** A dot after the label (for example: an update is available). */
  dot?: boolean;
}

/** Vertical tab list. */
export default function Tabs<Id extends string>({ tabs, active, onSelect }: { tabs: TabDef<Id>[]; active: Id; onSelect: (id: Id) => void }) {
  return (
    <nav className="tabs" role="tablist" aria-orientation="vertical">
      {tabs.map((t) => (
        <button key={t.id} role="tab" aria-selected={t.id === active} className={t.id === active ? "tab active" : "tab"} onClick={() => onSelect(t.id)}>
          {t.icon && <PhIcon name={t.icon} />}
          <span>{t.label}</span>
          {t.dot && <span className="tab-dot" aria-hidden="true" />}
        </button>
      ))}
    </nav>
  );
}
