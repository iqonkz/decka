import { useCallback, useEffect, useRef, useState } from "react";
import { locale, sameMinor, t, type DeviceInfo, type PairingInfo } from "@decka/shared";
import { call, onStatus } from "../ipc";
import { RELEASES_URL } from "../updates";
import Banner from "../ui/Banner";
import Button from "../ui/Button";
import { Group, Row, SectionHeader } from "../ui/SettingsGroup";
import Switch from "../ui/Switch";

const fmtCode = (code: string): string => `${code.slice(0, 4)}-${code.slice(4)}`;
const fmtTime = (ms: number): string => new Date(ms).toLocaleString(locale());

export default function PairingTab({ embedded = false }: { embedded?: boolean } = {}) {
  const [info, setInfo] = useState<PairingInfo | null>(null);
  const [remainingMs, setRemainingMs] = useState(0);
  const [expired, setExpired] = useState(false);
  const [pairedName, setPairedName] = useState<string | null>(null);
  const [devices, setDevices] = useState<DeviceInfo[]>([]);
  const [confirmId, setConfirmId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  // The version of this agent, to compare with the phones' (ARCHITECTURE.md 8a).
  const [agentVersion, setAgentVersion] = useState<string | null>(null);
  useEffect(() => {
    call("get_status").then((s) => setAgentVersion(s.version), () => {});
  }, []);

  // Devices known when the code was issued; a new id means the pairing succeeded.
  const knownIds = useRef<Set<string>>(new Set());
  const active = useRef(false);

  const refreshDevices = useCallback(async () => {
    const list = await call("list_devices");
    setDevices(list);
    if (active.current) {
      const fresh = list.find((d) => !knownIds.current.has(d.id));
      if (fresh) {
        active.current = false;
        setInfo(null);
        setExpired(false);
        setPairedName(fresh.name);
      }
    }
  }, []);

  useEffect(() => {
    refreshDevices().catch((e) => setError(String(e)));
    let off: (() => void) | null = null;
    let disposed = false;
    onStatus(() => void refreshDevices().catch(() => {})).then((unlisten) => {
      if (disposed) unlisten();
      else off = unlisten;
    });
    return () => {
      disposed = true;
      off?.();
      // Leaving the tab with a live code cancels it.
      if (active.current) {
        active.current = false;
        void call("cancel_pairing");
      }
    };
  }, [refreshDevices]);

  // Countdown.
  useEffect(() => {
    if (!info) return;
    const tick = () => {
      const left = info.expiresAt - Date.now();
      setRemainingMs(Math.max(0, left));
      if (left <= 0) {
        active.current = false;
        setInfo(null);
        setExpired(true);
      }
    };
    tick();
    const timer = window.setInterval(tick, 500);
    return () => window.clearInterval(timer);
  }, [info]);

  async function start() {
    setError(null);
    setPairedName(null);
    setExpired(false);
    try {
      knownIds.current = new Set((await call("list_devices")).map((d) => d.id));
      active.current = true;
      setInfo(await call("start_pairing"));
    } catch (e) {
      active.current = false;
      setError(String(e));
    }
  }

  async function cancel() {
    active.current = false;
    setInfo(null);
    await call("cancel_pairing");
  }

  async function revoke(id: string) {
    setConfirmId(null);
    await call("revoke_device", { deviceId: id });
    await refreshDevices();
  }

  return (
    <div className="stack">
      {!embedded && <h2>{t("pairing.title")}</h2>}
      {error && <Banner kind="error">{error}</Banner>}

      <section className="card">
        {info ? (
          <div className="pair-box">
            <div className="qr" dangerouslySetInnerHTML={{ __html: info.qrSvg }} />
            <div className="pair-info">
              <div className="pair-code">{fmtCode(info.code)}</div>
              <div className="muted">{t("pairing.expires", { s: Math.ceil(remainingMs / 1000) })}</div>
              <code className="pair-url">{info.url}</code>
              <p className="muted">
                {t("pairing.scan_hint")}
              </p>
              <Button onClick={() => void cancel()}>{t("common.cancel")}</Button>
            </div>
          </div>
        ) : (
          <div className="stack">
            {pairedName && <Banner kind="success">{t("pairing.paired", { name: pairedName })}</Banner>}
            {expired && <p className="muted">{t("pairing.expired")}</p>}
            <div>
              <Button variant="primary" onClick={() => void start()}>
                {t("pairing.pair_new")}
              </Button>
            </div>
          </div>
        )}
      </section>

      <SectionHeader title={t("pairing.your_phones")} />
      <Group>
        {devices.length === 0 ? (
          <div style={{ padding: "14px 0" }}>
            <p className="muted" style={{ margin: 0 }}>{t("pairing.none")}</p>
          </div>
        ) : (
          devices.map((d) => (
            <Row
              key={d.id}
              title={
                <span className="row" style={{ gap: 8 }}>
                  <span
                    className={d.connected ? "dot online" : "dot"}
                    title={d.connected ? t("pairing.online") : t("pairing.offline")}
                  />
                  <span>{d.name}</span>
                </span>
              }
              description={
                <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
                  <span>
                    {t("pairing.col_paired")}: {fmtTime(d.createdAt)} · {t("pairing.col_last_seen")}: {fmtTime(d.lastSeenAt)} ·{" "}
                    {t("pairing.col_version")}: {d.appVersion ?? t("pairing.version_unknown")}
                  </span>
                  {d.appVersion !== undefined && agentVersion !== null && !sameMinor(d.appVersion, agentVersion) && (
                    <span className="version-warn">
                      {t("pairing.update_recommended", { version: d.appVersion })}{" "}
                      <button type="button" className="link" onClick={() => void call("open_release", { url: RELEASES_URL })}>
                        {t("pairing.releases")}
                      </button>
                    </span>
                  )}
                </div>
              }
            >
              <div className="row" style={{ gap: 12, flexWrap: "wrap", justifyContent: "flex-end" }}>
                <label className="switch-row">
                  <Switch
                    checked={d.canApprove}
                    onChange={(allowed) => void call("set_can_approve", { deviceId: d.id, allowed }).then(refreshDevices)}
                    label={t("pairing.can_approve")}
                  />
                  <span>{t("pairing.can_approve")}</span>
                </label>
                {confirmId === d.id ? (
                  <span className="row">
                    <span>{t("pairing.forget_confirm", { name: d.name })}</span>
                    <Button variant="danger" onClick={() => void revoke(d.id)}>
                      {t("pairing.forget")}
                    </Button>
                    <Button onClick={() => setConfirmId(null)}>{t("common.cancel")}</Button>
                  </span>
                ) : (
                  <Button onClick={() => setConfirmId(d.id)}>{t("pairing.forget")}</Button>
                )}
              </div>
            </Row>
          ))
        )}
      </Group>
    </div>
  );
}
