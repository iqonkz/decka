import { useEffect, useState } from "react";
import { t, type NetworkStatus } from "@decka/shared";
import { call } from "../ipc";
import Banner from "../ui/Banner";
import { Group, Row } from "../ui/SettingsGroup";
import Switch from "../ui/Switch";

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
    <div style={{ marginTop: 16 }}>
      {status.kind === "public" && (
        <div style={{ marginBottom: 12 }}>
          <Banner kind="warning">{t("phone.net.public")}</Banner>
        </div>
      )}
      <Group>
        {status.kind !== "public" && (
          <Row title={t(`phone.net.${status.kind}`)} />
        )}
        <Row title={t("phone.net.allow_public")}>
          <Switch checked={status.allowPublic} onChange={(val) => void setAllowPublic(val)} label={t("phone.net.allow_public")} />
        </Row>
      </Group>
    </div>
  );
}
