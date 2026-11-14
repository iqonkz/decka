export interface TabDef<Id extends string> {
  id: Id;
  label: string;
}

/** Vertical tab list. */
export default function Tabs<Id extends string>({ tabs, active, onSelect }: { tabs: TabDef<Id>[]; active: Id; onSelect: (id: Id) => void }) {
  return (
    <nav className="tabs" role="tablist" aria-orientation="vertical">
      {tabs.map((t) => (
        <button key={t.id} role="tab" aria-selected={t.id === active} className={t.id === active ? "tab active" : "tab"} onClick={() => onSelect(t.id)}>
          {t.label}
        </button>
      ))}
    </nav>
  );
}
