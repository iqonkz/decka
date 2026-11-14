import { useEffect, useState } from "react";
import type { NotificationsItem } from "@decka/shared";
import { dismiss } from "../../app/controller";
import { useAppState } from "../../store/store";
import { relTime } from "./relTime";
import { LEVEL_COLOR } from "./statusOrder";

const REFRESH_MS = 30_000;

export default function Notifications(_props: { item: NotificationsItem }) {
  const list = useAppState((s) => s.state?.notifications);
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), REFRESH_MS);
    return () => window.clearInterval(timer);
  }, []);

  if (!list || list.length === 0) return <div className="tile-body notif-empty">No notifications</div>;
  return (
    // Vertical scrolling stays inside the widget: its pointerdown never reaches the page swipe handler.
    <div className="notifs" onPointerDown={(e) => e.stopPropagation()}>
      {list.map((n) => (
        <div className="notif" key={n.id} style={{ borderLeftColor: LEVEL_COLOR[n.level] }} onClick={() => dismiss(n.id)}>
          <div className="notif-main">
            <div className="notif-title">{n.title}</div>
            {n.body && <div className="notif-body">{n.body}</div>}
          </div>
          <div className="notif-meta">{[n.source, relTime(n.ts, now)].filter(Boolean).join(" · ")}</div>
        </div>
      ))}
    </div>
  );
}
