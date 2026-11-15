import { useState } from "react";
import { chordFromEvent } from "./keyName";
import { formatChord } from "./addButton";

/**
 * `Click and press keys`: the field records the chord that is pressed (DESIGN.md 4.6a); there is no typing of `ctrl+shift+m`.
 * `keys` is the recorded chord (DATA_MODEL.md 2.2 names); an unsupported key gives the hint `This key is not supported`.
 */
export default function ChordField({ keys, onChange }: { keys: string[]; onChange: (keys: string[]) => void }) {
  const [hint, setHint] = useState<string | null>(null);
  const [recording, setRecording] = useState(false);
  return (
    <div>
      <button
        type="button"
        className={`chord-field${recording ? " recording" : ""}`}
        onFocus={() => setRecording(true)}
        onBlur={() => setRecording(false)}
        onKeyDown={(e) => {
          e.preventDefault();
          e.stopPropagation();
          if (e.key === "Tab") return;
          const next = chordFromEvent(e.nativeEvent);
          setHint(next.unsupported ? "This key is not supported" : null);
          if (next.keys.length > 0) onChange(next.keys.slice(0, 5));
        }}
      >
        {keys.length > 0 ? formatChord(keys) : recording ? "Press the keys…" : "Click and press keys"}
      </button>
      {hint && <small className="warn-text">{hint}</small>}
    </div>
  );
}