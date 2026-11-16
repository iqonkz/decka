import type { SysItem } from "@decka/shared";
import { useDeckEnv } from "../deckEnv";
import { formatRate, percent } from "./format";
import { sparklinePath } from "./sparkline";
import { useSysHistory } from "./sysHistory";

function barColor(value: number): string {
  if (value >= 95) return "#dc2626";
  if (value >= 85) return "#d97706";
  return "#2563eb";
}

function Row({ label, value }: { label: string; value: number }) {
  return (
    <div className="sys-row">
      <span className="sys-label">{label}</span>
      <span className="sys-bar">
        <span className="sys-fill" style={{ width: `${Math.min(100, value)}%`, background: barColor(value) }} />
      </span>
      <span className="sys-value">{value}%</span>
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
function Graph({ item }: { item: SysItem }) {
  const { sys } = useDeckEnv();
  const history = useSysHistory();
  if (!sys) return <div className="tile-body">—</div>;
  const ram = percent(sys.mem.usedMb, sys.mem.totalMb);
  const disk = sys.disks[0];
  return (
    <div className="sys-graph">
      <div className="sys-graph-head">
        <span className="cpu">CPU {sys.cpu}%</span>
        <span className="ram">RAM {ram}%</span>
      </div>
      <svg className="sys-graph-plot" viewBox={`0 0 ${GRAPH_W} ${GRAPH_H}`} preserveAspectRatio="none" aria-hidden="true">
        <Line values={history.ram} color="var(--graph-ram, #a855f7)" />
        <Line values={history.cpu} color="var(--graph-cpu, #3b82f6)" />
      </svg>
      {item.h >= 2 && (
        <div className="sys-net">
          {disk ? `${disk.mount} ${percent(disk.usedGb, disk.totalGb)}%  ` : ""}↓ {formatRate(sys.net.rxKbps)}&nbsp;&nbsp;↑ {formatRate(sys.net.txKbps)}
        </div>
      )}
    </div>
  );
}

export default function Sys({ item }: { item: SysItem }) {
  const { sys } = useDeckEnv();
  if (item.graph === true) return <Graph item={item} />;
  if (!sys) return <div className="tile-body">—</div>;

  const disk = sys.disks[0];
  return (
    <div className="sys">
      <Row label="CPU" value={sys.cpu} />
      <Row label="RAM" value={percent(sys.mem.usedMb, sys.mem.totalMb)} />
      {item.h >= 2 && disk && <Row label={disk.mount} value={percent(disk.usedGb, disk.totalGb)} />}
      {item.h >= 2 && (
        <div className="sys-net">
          ↓ {formatRate(sys.net.rxKbps)}&nbsp;&nbsp;↑ {formatRate(sys.net.txKbps)}
        </div>
      )}
    </div>
  );
}
