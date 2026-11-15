import type { SysItem } from "@decka/shared";
import { useDeckEnv } from "../deckEnv";
import { formatRate, percent } from "./format";

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

export default function Sys({ item }: { item: SysItem }) {
  const { sys } = useDeckEnv();
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
