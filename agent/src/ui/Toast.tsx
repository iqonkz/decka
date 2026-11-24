import { useEffect, useState } from "react";
import PhIcon from "./PhIcon";

export interface ToastInfo {
  id: number;
  message: string;
  kind?: "success" | "danger";
}

let listeners: Array<(toast: ToastInfo | null) => void> = [];
let currentToast: ToastInfo | null = null;
let toastTimer: ReturnType<typeof setTimeout> | null = null;

export function showToast(message: string, kind: "success" | "danger" = "success", durationMs = 1500) {
  if (toastTimer) clearTimeout(toastTimer);
  const toast: ToastInfo = { id: Date.now(), message, kind };
  currentToast = toast;
  listeners.forEach((l) => l(toast));
  toastTimer = setTimeout(() => {
    if (currentToast?.id === toast.id) {
      currentToast = null;
      listeners.forEach((l) => l(null));
    }
  }, durationMs);
}

export function ToastContainer() {
  const [toast, setToast] = useState<ToastInfo | null>(currentToast);

  useEffect(() => {
    listeners.push(setToast);
    return () => {
      listeners = listeners.filter((l) => l !== setToast);
    };
  }, []);

  if (!toast) return null;

  return (
    <div className={`settings-toast ${toast.kind ?? "success"}`} role="status" aria-live="polite">
      <PhIcon name={toast.kind === "danger" ? "warning-circle" : "check"} size={16} />
      <span>{toast.message}</span>
    </div>
  );
}

export default ToastContainer;
