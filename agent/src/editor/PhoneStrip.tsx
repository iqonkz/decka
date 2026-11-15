import { useEffect, useState } from "react";
import { t } from "@decka/shared";
import { call } from "../ipc";

/** `Phone not connected — Pair a phone` over the preview, instead of silence (DESIGN.md 4.7). Disappears when a phone is connected. */
export default function PhoneStrip() {
  const [connected, setConnected] = useState<boolean | null>(null);
  useEffect(() => {
    let live = true;
    const poll = () =>
      call("get_status").then(
        (s) => live && setConnected(s.clients.length > 0),
        () => {},
      );
    void poll();
    const timer = window.setInterval(poll, 3000);
    return () => {
      live = false;
      window.clearInterval(timer);
    };
  }, []);
  if (connected !== false) return null;
  return (
    <div className="phone-strip" role="status">
      {t("strip.not_connected")} —{" "}
      <button type="button" className="link" onClick={() => window.dispatchEvent(new CustomEvent("decka:go", { detail: { tab: "pairing" } }))}>
        {t("strip.pair")}
      </button>
    </div>
  );
}