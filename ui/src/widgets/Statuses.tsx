import { useEffect, useRef, useState } from "react";
import { t, type StatusesItem } from "@decka/shared";
import { useDeckEnv } from "../deckEnv";
import { hasWidgetHeader } from "./density";
import { sortStatuses, STATE_COLOR } from "./statusOrder";
import WidgetEmpty from "./WidgetEmpty";

const ROW_PX = 28;

export default function Statuses({ item }: { item: StatusesItem }) {
  const { statuses } = useDeckEnv();
  const box = useRef<HTMLDivElement>(null);
  const titleEl = useRef<HTMLDivElement>(null);
  const [capacity, setCapacity] = useState(3);

  useEffect(() => {
    const el = box.current;
    if (!el) return;
    const measure = () => {
      const free = el.clientHeight - (titleEl.current?.offsetHeight ?? 0);
      setCapacity(Math.max(1, Math.floor(free / ROW_PX)));
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  const list = sortStatuses(statuses, item.group);
  const overflow = list.length > capacity;
  const shown = overflow ? list.slice(0, Math.max(0, capacity - 1)) : list;

  return (
    <div className="statuses" ref={box}>
      {!hasWidgetHeader(item) && item.title && (
        <div className="statuses-title" ref={titleEl}>
          {item.title}
        </div>
      )}
      {list.length === 0 ? (
        <WidgetEmpty icon="lucide:activity" text={t("widget.no_statuses")} />
      ) : (
        <>
          {shown.map((s) => (
            <div className="status-row" key={s.id}>
              <span className={s.state === "running" ? "status-dot pulse" : "status-dot"} style={{ background: STATE_COLOR[s.state] }} />
              <span className="status-label">{s.label}</span>
              {s.text && <span className="status-text">{s.text}</span>}
            </div>
          ))}
          {overflow && <div className="status-more">{t("widget.more", { n: list.length - shown.length })}</div>}
        </>
      )}
    </div>
  );
}
