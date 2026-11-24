import { t, type SysItem } from "@decka/shared";
import { useDeckEnv } from "../deckEnv";
import { formatRate, percent } from "./format";
import { sparklinePath } from "./sparkline";
import { useSysHistory } from "./sysHistory";
import { useAnimatedNumber } from "./useAnimatedNumber";
import WidgetEmpty from "./WidgetEmpty";

function barColor(value: number): string {
  if (value >= 95) return "#ef4444";
  if (value >= 85) return "#f59e0b";
  return "var(--accent, #7c5cff)";
}

function Row({ label, value, extraClass }: { label: string; value: number; extraClass?: string }) {
  const shown = useAnimatedNumber(value, 300);
  return (
    <div className={extraClass ? `sys-row ${extraClass}` : "sys-row"}>
      <span className="sys-label">{label}</span>
      <span className="sys-bar">
        <span className="sys-fill" style={{ width: `${Math.min(100, value)}%`, background: barColor(value) }} />
      </span>
      <span className="sys-value">{shown}%</span>
    </div>
  );
}

const GRAPH_W = 118;
const GRAPH_H = 40;

/** The line of one reading and the area under it (DATA_MODEL.md 6.5); the box stretches to the widget. */
function Line({ values, color }: { values: number[]; color: string }) {
  const { line, area } = sparklinePath(values, GRAPH_W, GRAPH_H);
  if (!line) return null;
  return (
    <>
      <path d={area} fill={color} fillOpacity={0.15} stroke="none" />
      <path d={line} fill="none" stroke={color} strokeWidth={1.5} strokeLinejoin="round" strokeLinecap="round" vectorEffect="non-scaling-stroke" />
    </>
  );
}

/** `sys` with `graph: true`: CPU and RAM over the last two minutes, the current numbers in the top right corner. */
function Graph() {
  const { sys } = useDeckEnv();
  const history = useSysHistory();
  const cpuShown = useAnimatedNumber(sys?.cpu ?? 0, 300);
  const ramPct = sys ? percent(sys.mem.usedMb, sys.mem.totalMb) : 0;
  const ramShown = useAnimatedNumber(ramPct, 300);
  if (!sys) return <WidgetEmpty icon="lucide:cpu" text={t("widget.no_data")} />;
  const disk = sys.disks[0];
  return (
    <div className="sys-graph">
      <div className="sys-graph-head">
        <span className="cpu">CPU {cpuShown}%</span>
        <span className="ram">RAM {ramShown}%</span>
      </div>
      <svg className="sys-graph-plot" viewBox={`0 0 ${GRAPH_W} ${GRAPH_H}`} preserveAspectRatio="none" aria-hidden="true">
        <Line values={history.ram} color="var(--graph-ram, #a855f7)" />
        <Line values={history.cpu} color="var(--graph-cpu, #3b82f6)" />
      </svg>
      <div className="sys-net w-l">
        {disk ? `${disk.mount} ${percent(disk.usedGb, disk.totalGb)}% · ` : ""}↓ {formatRate(sys.net.rxKbps)} · ↑ {formatRate(sys.net.txKbps)}
      </div>
    </div>
  );
}

/** System widget (DESIGN.md 4.16): container queries control S (CPU/RAM), M (+disk), L (+network). */
export default function Sys({ item }: { item: SysItem }) {
  const { sys } = useDeckEnv();
  if (item.graph === true) return <Graph />;
  if (!sys) return <WidgetEmpty icon="lucide:cpu" text={t("widget.no_data")} />;

  const disk = sys.disks[0];
  const ram = percent(sys.mem.usedMb, sys.mem.totalMb);
  return (
    <div className="sys">
      <Row label="CPU" value={sys.cpu} />
      <Row label="RAM" value={ram} />
      {disk && <Row label={disk.mount} value={percent(disk.usedGb, disk.totalGb)} extraClass="w-m" />}
      <div className="sys-net w-l">
        ↓ {formatRate(sys.net.rxKbps)}&nbsp;&nbsp;↑ {formatRate(sys.net.txKbps)}
      </div>
    </div>
  );
}
