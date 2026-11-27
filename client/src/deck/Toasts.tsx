import { useEffect, useRef, useState } from "react";
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

const SWIPE_DISMISS_PX = 48;

/**
 * Bottom pill toasts (M31 T31.8b): at most 2 visible, 12 px radius, typed icon on left,
 * pointer swipe-to-dismiss.
 */
export default function Toasts() {
  const toasts = useAppState((s) => s.toasts).slice(-2);
  const dragStart = useRef<{ id: number; x: number; y: number } | null>(null);
  const [dragOffset, setDragOffset] = useState<{ id: number; dx: number; dy: number } | null>(null);

  useEffect(() => {
    const timer = window.setInterval(() => expireToasts(Date.now()), 500);
    return () => window.clearInterval(timer);
  }, []);

  if (toasts.length === 0) return null;
  return (
    <div className="toasts">
      {toasts.map((item) => {
        const activeDrag = dragOffset?.id === item.id ? dragOffset : null;
        const opacity = activeDrag ? Math.max(0.25, 1 - (Math.abs(activeDrag.dx) + Math.max(0, activeDrag.dy)) / 140) : 1;
        return (
          <div
            key={item.id}
            className="toast"
            style={
              activeDrag
                ? { transform: `translate3d(${activeDrag.dx}px, ${Math.max(0, activeDrag.dy)}px, 0)`, opacity, transition: "none" }
                : undefined
            }
            onPointerDown={(e) => {
              dragStart.current = { id: item.id, x: e.clientX, y: e.clientY };
            }}
            onPointerMove={(e) => {
              const s = dragStart.current;
              if (!s || s.id !== item.id) return;
              const dx = e.clientX - s.x;
              const dy = e.clientY - s.y;
              if (Math.hypot(dx, dy) > 8) setDragOffset({ id: item.id, dx, dy });
            }}
            onPointerUp={(e) => {
              const s = dragStart.current;
              dragStart.current = null;
              setDragOffset(null);
              if (!s || s.id !== item.id) return;
              const dx = e.clientX - s.x;
              const dy = e.clientY - s.y;
              if (Math.abs(dx) >= SWIPE_DISMISS_PX || dy >= SWIPE_DISMISS_PX) {
                dismissToast(item.id);
                return;
              }
              // A swipe that was taken back is not a tap: the toast stays and nothing opens on the PC.
              if (Math.hypot(dx, dy) > 8) return;
              if (item.notifId !== undefined) openNotification(item.notifId);
              dismissToast(item.id);
            }}
            onPointerCancel={() => {
              dragStart.current = null;
              setDragOffset(null);
            }}
          >
            <PhIcon name={LEVEL_ICON[item.level]} size={20} color={LEVEL_COLOR[item.level]} className="toast-icon" />
            <div className="toast-copy">
              <span className="toast-title">{item.title}</span>
              {item.body && <span className="toast-body">{item.body}</span>}
            </div>
          </div>
        );
      })}
    </div>
  );
}
