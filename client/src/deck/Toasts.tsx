import { useEffect } from "react";
import type { NotifLevel } from "@decka/shared";
import { openNotification } from "../app/controller";
import { dismissToast, expireToasts, useAppState } from "../store/store";
import { LEVEL_COLOR } from "@decka/ui";
import PhIcon, { type PhIconName } from "../ui/PhIcon";

const LEVEL_ICON: Record<NotifLevel, PhIconName> = {
  info: "info",
  success: "check-circle",
  warning: "warning",
  error: "x-circle",
};

/** Top pill toasts (DESIGN.md 4.16): state icon on the left, 95% theme surface + blur 16, max 2 lines. */
export default function Toasts() {
  const toasts = useAppState((s) => s.toasts);

  useEffect(() => {
    const timer = window.setInterval(() => expireToasts(Date.now()), 500);
    return () => window.clearInterval(timer);
  }, []);

  if (toasts.length === 0) return null;
  return (
    <div className="toasts">
      {toasts.map((item) => (
        <div
          key={item.id}
          className="toast"
          onPointerUp={() => {
            // A toast of a Windows notification is a tap on that notification: the PC opens its program.
            if (item.notifId !== undefined) openNotification(item.notifId);
            dismissToast(item.id);
          }}
        >
          <PhIcon name={LEVEL_ICON[item.level]} size={20} color={LEVEL_COLOR[item.level]} className="toast-icon" />
          <div className="toast-copy">
            <span className="toast-title">{item.title}</span>
            {item.body && <span className="toast-body">{item.body}</span>}
          </div>
        </div>
      ))}
    </div>
  );
}
