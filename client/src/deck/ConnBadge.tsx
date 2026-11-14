import { useEffect, useState } from "react";
import { useAppState } from "../store/store";

const OFFLINE_GRACE_MS = 3000;

/** Top-right connection indicator. Short drops (< 3 s) show nothing, to avoid flicker. */
export default function ConnBadge() {
  const conn = useAppState((s) => s.conn);
  const [showOffline, setShowOffline] = useState(false);

  useEffect(() => {
    if (conn !== "offline") {
      setShowOffline(false);
      return;
    }
    const timer = window.setTimeout(() => setShowOffline(true), OFFLINE_GRACE_MS);
    return () => window.clearTimeout(timer);
  }, [conn]);

  if (conn === "connecting") return <div className="badge connecting">Connecting…</div>;
  if (conn === "offline" && showOffline) return <div className="badge offline">PC offline — retrying</div>;
  return null;
}
