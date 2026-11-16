import { useState } from "react";
import { t, tn } from "@decka/shared";
import { gridLimits, type Orientation } from "@decka/shared";

interface Props {
  cols: number;
  rows: number;
  /** The limits follow the orientation of the deck (DATA_MODEL.md 3.2 rule 1); absent = landscape. */
  orientation?: Orientation | undefined;
  onPick: (cols: number, rows: number) => void;
}

/** `Grid 7 × 4`: a button that opens a mini grid 8x5 to choose a size by pointing, like the table size of a text editor. */
export default function GridPicker({ cols, rows, orientation, onPick }: Props) {
  const [open, setOpen] = useState(false);
  const [hover, setHover] = useState<{ c: number; r: number } | null>(null);
  const shown = hover ?? { c: cols, r: rows };
  const limits = gridLimits(orientation);
  const [minC, maxC] = limits.cols;
  const [minR, maxR] = limits.rows;

  return (
    <div className="grid-picker">
      <button type="button" className="btn" aria-haspopup="true" aria-expanded={open} onClick={() => setOpen((v) => !v)}>
        {t("grid.button", { cols, rows })}
      </button>
      {open && (
        <>
          <div className="menu-backdrop" onPointerDown={() => setOpen(false)} />
          <div className="grid-pop" role="dialog" aria-label={t("grid.size")} onPointerLeave={() => setHover(null)}>
            <div className="mini-grid" style={{ gridTemplateColumns: `repeat(${maxC}, 22px)` }}>
              {Array.from({ length: maxR }, (_, ri) =>
                Array.from({ length: maxC }, (_, ci) => {
                  const c = ci + 1;
                  const r = ri + 1;
                  const allowed = c >= minC && r >= minR;
                  return (
                    <button
                      key={`${c}-${r}`}
                      type="button"
                      aria-label={t("grid.cell", { c, r })}
                      disabled={!allowed}
                      className={`mini-cell${c <= shown.c && r <= shown.r ? " on" : ""}`}
                      onPointerEnter={() => allowed && setHover({ c, r })}
                      onClick={() => {
                        setOpen(false);
                        onPick(c, r);
                      }}
                    />
                  );
                }),
              )}
            </div>
            <div className="muted mini-caption">
              {shown.c} × {shown.r} · {tn("grid.keys", shown.c * shown.r)}
            </div>
          </div>
        </>
      )}
    </div>
  );
}