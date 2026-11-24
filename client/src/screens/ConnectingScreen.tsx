import { useEffect, useState } from "react";
import { t } from "@decka/shared";
import { gridMetrics, Skeleton } from "@decka/ui";
import { savedOrientation } from "../platform/orientation";
import { useAppState } from "../store/store";

/**
 * Before the first state arrives: skeleton panel grid (30% panels with 1.2s shimmer, DESIGN.md 4.16)
 * plus subtle connecting status pill. After 8 s without connection LinkCard appears over it (DESIGN.md 4.13).
 */
export default function ConnectingScreen() {
  const conn = useAppState((s) => s.conn);
  const error = useAppState((s) => s.lastError);
  const [vp, setVp] = useState(() => ({ w: window.innerWidth, h: window.innerHeight }));
  useEffect(() => {
    const onResize = () => setVp({ w: window.innerWidth, h: window.innerHeight });
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, []);

  const portrait = savedOrientation() === "portrait" || vp.h > vp.w;
  const cols = portrait ? 4 : 7;
  const rows = portrait ? 7 : 4;
  const m = gridMetrics(vp, cols, rows);
  const total = cols * rows;

  return (
    <div className="deck connecting-screen">
      <div
        className="grid-inner"
        style={{
          left: m.offsetX,
          top: m.offsetY,
          ["--side" as string]: `${m.side}px`,
          gridTemplateColumns: `repeat(${cols}, ${m.side}px)`,
          gridTemplateRows: `repeat(${rows}, ${m.side}px)`,
          gap: m.gap,
        }}
      >
        {Array.from({ length: total }, (_, i) => (
          <Skeleton key={i} radius={Math.round(0.22 * m.side)} />
        ))}
      </div>
      <div className="connecting-status">
        <span>{conn === "offline" ? t("phone.no_pc") : t("phone.connecting")}</span>
        {error && <span className="connecting-error">{error}</span>}
      </div>
    </div>
  );
}
