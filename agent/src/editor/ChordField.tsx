import { useEffect, useState } from "react";
import { t } from "@decka/shared";
import { chordFromEvent } from "./keyName";
import { formatChord } from "./addButton";
import { chordToState, keyLabel, MODIFIERS, searchKeys, stateToChord, type ChordState, type Modifier } from "./chord";

const MODIFIER_LABELS: Record<Modifier, string> = { ctrl: "Ctrl", alt: "Alt", shift: "Shift", win: "Win" };

/**
 * `Click and press keys` records the chord that is pressed (DESIGN.md 4.6a). Under it the same value can be chosen by hand
 * (DESIGN.md 4.9): four modifier switches and a searchable key, for the shortcuts Windows takes before the window sees them.
 * `keys` is the chord (DATA_MODEL.md 2.2 names); an unsupported key gives the hint `This key is not supported`.
 */
export default function ChordField({ keys, onChange, compact = false }: { keys: string[]; onChange: (keys: string[]) => void; compact?: boolean }) {
  const [hint, setHint] = useState<string | null>(null);
  const [recording, setRecording] = useState(false);
  const [draft, setDraft] = useState<ChordState>(() => chordToState(keys));
  const [query, setQuery] = useState("");
  const [open, setOpen] = useState(false);

  // A recorded chord (or another action) arrives through `keys`: the switches follow it.
  const joined = keys.join("+");
  useEffect(() => {
    setDraft(chordToState(keys));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [joined]);

  function apply(next: ChordState) {
    setDraft(next);
    const chord = stateToChord(next);
    if (chord.ok) {
      setHint(null);
      onChange(chord.keys);
    } else {
      setHint(t("chord.pick_key")); // nothing is saved until a key is chosen
    }
  }

  const groups = searchKeys(query);
  const total = stateToChord(draft);
  const pickKey = (key: string) => {
    setQuery("");
    setOpen(false);
    apply({ ...draft, key });
  };

  return (
    <div>
      {!compact && <button
        type="button"
        className={`chord-field${recording ? " recording" : ""}`}
        onFocus={() => setRecording(true)}
        onBlur={() => setRecording(false)}
        onKeyDown={(e) => {
          e.preventDefault();
          e.stopPropagation();
          if (e.key === "Tab") return;
          const next = chordFromEvent(e.nativeEvent);
          setHint(next.unsupported ? t("picker.key_unsupported") : null);
          if (next.keys.length > 0) onChange(next.keys.slice(0, 5));
        }}
      >
        {keys.length > 0 ? formatChord(keys) : recording ? t("chord.recording") : t("chord.idle")}
      </button>}
      <div className="chord-manual">
        {MODIFIERS.map((m) => (
          <button key={m} type="button" className={`chord-mod${draft[m] ? " on" : ""}`} aria-pressed={draft[m]} onClick={() => apply({ ...draft, [m]: !draft[m] })}>
            {MODIFIER_LABELS[m]}
          </button>
        ))}
        <span className="chord-plus">+</span>
        <div className="chord-key">
          <input
            type="text"
            value={open ? query : draft.key ? keyLabel(draft.key) : ""}
            placeholder={t("chord.search")}
            onFocus={() => {
              setQuery("");
              setOpen(true);
            }}
            onBlur={() => window.setTimeout(() => setOpen(false), 120)} // after a click on a list entry has landed
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => {
              e.stopPropagation();
              if (e.key === "Enter") {
                e.preventDefault();
                const first = groups[0]?.keys[0];
                if (first) pickKey(first);
              } else if (e.key === "Escape") setOpen(false);
            }}
          />
          {open && (
            <div className="chord-list" role="listbox">
              {groups.length === 0 && <div className="chord-empty">{t("chord.no_match")}</div>}
              {groups.map((g) => (
                <div key={g.id}>
                  <div className="chord-group">{t(`chord.group.${g.id}`)}</div>
                  {g.keys.map((k) => (
                    <button key={k} type="button" role="option" aria-selected={draft.key === k} className={draft.key === k ? "on" : ""} onMouseDown={(e) => e.preventDefault()} onClick={() => pickKey(k)}>
                      {keyLabel(k)}
                    </button>
                  ))}
                </div>
              ))}
            </div>
          )}
        </div>
      </div>
      <small className="muted">{t("chord.hint")}</small>
      {!compact && total.ok && total.keys.length > 0 && <div className="chord-total">{formatChord(total.keys)}</div>}
      {hint && <small className="warn-text">{hint}</small>}
    </div>
  );
}
