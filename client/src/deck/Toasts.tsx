import { useEffect } from "react";
import type { NotifLevel } from "@decka/shared";
import { dismissToast, expireToasts, useAppState } from "../store/store";

const LEVEL_COLORS: Record<NotifLevel, string> = {
  info: "#2563eb",
  success: "#16a34a",
  warning: "#d97706",
  error: "#dc2626",
};

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
          style={{ borderLeftColor: LEVEL_COLORS[t.level] }}
          onPointerUp={() => dismissToast(t.id)}
        >
          <div className="toast-title">{t.title}</div>
          {t.body && <div className="toast-body">{t.body}</div>}
        </div>
      ))}
    </div>
  );
}
