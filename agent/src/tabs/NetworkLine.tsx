import { useEffect, useState } from "react";
import { t, type NetworkStatus } from "@decka/shared";
import { call } from "../ipc";

/** `Phone` page (DATA_MODEL.md 11.2): the kind of network the phones use and `Allow phones on public networks`. */
export default function NetworkLine() {
  const [status, setStatus] = useState<NetworkStatus | null>(null);

  useEffect(() => {
    let live = true;
    const ask = () => void call("network_status").then((s) => live && setStatus(s), () => {});
    ask();
    const timer = window.setInterval(ask, 5000);
    return () => {
      live = false;
      window.clearInterval(timer);
    };
  }, []);

  async function setAllowPublic(allowPublic: boolean) {
    const fresh = await call("get_config");
    const result = await call("save_config", { config: { ...fresh, network: { allowPublic } } });
    if (result.ok) setStatus(await call("network_status"));
  }

  if (!status) return null;
  return (
    <section className="card">
      <div className="row">
        <strong className={status.kind === "public" ? "warn-text grow" : "grow"}>{t(`phone.net.${status.kind}`)}</strong>
      </div>
      <label className="check block">
        <input type="checkbox" checked={status.allowPublic} onChange={(e) => void setAllowPublic(e.target.checked)} /> {t("phone.net.allow_public")}
      </label>
    </section>
  );
}
