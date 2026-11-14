import { useEffect, useState } from "react";
import { useAppState } from "../store/store";

const OFFLINE_GRACE_MS = 3000;
const CHANNEL_SHOWN_MS = 4000;

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

  const via = useAppState((s) => s.via);
  const [channelShown, setChannelShown] = useState(false);

  // The channel (USB or Wi-Fi) is named for 4 s after each connection, then hidden: no permanent labels (burn-in).
  useEffect(() => {
    if (conn !== "online" || via === null) {
      setChannelShown(false);
      return;
    }
    setChannelShown(true);
    const timer = window.setTimeout(() => setChannelShown(false), CHANNEL_SHOWN_MS);
    return () => window.clearTimeout(timer);
  }, [conn, via]);

  if (conn === "online" && channelShown && via !== null) return <div className="badge channel">{via === "usb" ? "USB" : "Wi-Fi"}</div>;
  if (conn === "connecting") return <div className="badge connecting">Connecting…</div>;
  if (conn === "offline" && showOffline) return <div className="badge offline">PC offline — retrying</div>;
  return null;
}
