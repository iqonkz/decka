import { useEffect } from "react";
import { dismissToast, expireToasts, useAppState } from "../store/store";
import { LEVEL_COLOR } from "./widgets/statusOrder";


export default function Toasts() {
  const toasts = useAppState((s) => s.toasts);

  useEffect(() => {
    const timer = window.setInterval(() => expireToasts(Date.now()), 500);
    return () => window.clearInterval(timer);
  }, []);

  if (toasts.length === 0) return null;
  return (
    <div className="toasts">
      {toasts.map((t) => (
        <div
          key={t.id}
          className="toast"
          style={{ borderLeftColor: LEVEL_COLOR[t.level] }}
          onPointerUp={() => dismissToast(t.id)}
        >
          <div className="toast-title">{t.title}</div>
          {t.body && <div className="toast-body">{t.body}</div>}
        </div>
      ))}
    </div>
  );
}
