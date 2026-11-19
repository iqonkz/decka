import { t, type BatteryItem } from "@decka/shared";
import { useDeckEnv } from "../deckEnv";
import Icon from "../Icon";
import { batteryIcon, chargeLevel } from "./batteryIcon";

const COLOR = { error: "var(--error, #ef4444)", warning: "var(--warning, #f59e0b)", ok: "currentColor" } as const;

/**
 * The charge of the PC and of Bluetooth devices (DATA_MODEL.md 10.6). 1x1 shows the most drained device; larger sizes list the
 * devices: icon, name, bar, percent. Below 20% the colour is `warning`, below 10% `error`.
 */
export default function Battery({ item }: { item: BatteryItem }) {
  const { battery } = useDeckEnv();
  const devices = battery?.devices ?? [];
  if (devices.length === 0) {
    return (
      <div className="battery battery-empty">
        <Icon icon="lucide:bluetooth" size={20} />
        <span className="battery-none">{t("battery.none")}</span>
      </div>
    );
  }
  if (item.w === 1 && item.h === 1) {
    const worst = devices.reduce((a, b) => (b.pct < a.pct ? b : a));
    return (
      <div className="battery battery-one" style={{ color: COLOR[chargeLevel(worst.pct)] }}>
        <Icon icon={batteryIcon(worst.kind, worst.name)} size={22} />
        <strong>{worst.pct}%</strong>
      </div>
    );
  }
  const rows = item.h === 1 ? 2 : 5;
  return (
    <div className="battery battery-list">
      {devices.slice(0, rows).map((d) => (
        <div className="battery-row" key={`${d.kind}:${d.name}`} style={{ color: COLOR[chargeLevel(d.pct)] }}>
          <Icon icon={batteryIcon(d.kind, d.name)} size={16} />
          <span className="battery-name">{d.kind === "pc" ? t("battery.pc") : d.name}</span>
          <span className="battery-bar">
            <span className="battery-fill" style={{ width: `${d.pct}%` }} />
          </span>
          <span className="battery-pct">{d.pct}%</span>
        </div>
      ))}
    </div>
  );
}
