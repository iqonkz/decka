import { useCallback, useEffect, useRef, useState } from "react";
import type { DeviceInfo, PairingInfo } from "@decka/shared";
import { call, onStatus } from "../ipc";
import Banner from "../ui/Banner";
import Button from "../ui/Button";

const fmtCode = (code: string): string => `${code.slice(0, 4)}-${code.slice(4)}`;
const fmtTime = (ms: number): string => new Date(ms).toLocaleString();

export default function PairingTab({ embedded = false }: { embedded?: boolean } = {}) {
  const [info, setInfo] = useState<PairingInfo | null>(null);
  const [remainingMs, setRemainingMs] = useState(0);
  const [expired, setExpired] = useState(false);
  const [pairedName, setPairedName] = useState<string | null>(null);
  const [devices, setDevices] = useState<DeviceInfo[]>([]);
  const [confirmId, setConfirmId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

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
      {!embedded && <h2>Pairing</h2>}
      {error && <Banner kind="error">{error}</Banner>}

      <section className="card">
        {info ? (
          <div className="pair-box">
            <div className="qr" dangerouslySetInnerHTML={{ __html: info.qrSvg }} />
            <div className="pair-info">
              <div className="pair-code">{fmtCode(info.code)}</div>
              <div className="muted">Expires in {Math.ceil(remainingMs / 1000)} s</div>
              <code className="pair-url">{info.url}</code>
              <p className="muted">
                Scan with the Decka app or the phone camera. Phone and PC must be on the same Wi-Fi, or connected by USB with USB mode enabled.
              </p>
              <Button onClick={() => void cancel()}>Cancel</Button>
            </div>
          </div>
        ) : (
          <div className="stack">
            {pairedName && <Banner kind="success">Paired: {pairedName}</Banner>}
            {expired && <p className="muted">Code expired</p>}
            <div>
              <Button variant="primary" onClick={() => void start()}>
                Pair a new phone
              </Button>
            </div>
          </div>
        )}
      </section>

      <section className="card">
        <h3>Your phones</h3>
        {devices.length === 0 ? (
          <p className="muted">No phones paired yet.</p>
        ) : (
          <table>
            <thead>
              <tr>
                <th />
                <th>Name</th>
                <th>Paired</th>
                <th>Last seen</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {devices.map((d) => (
                <tr key={d.id}>
                  <td>
                    <span className={d.connected ? "dot online" : "dot"} title={d.connected ? "online" : "offline"} />
                  </td>
                  <td>{d.name}</td>
                  <td>{fmtTime(d.createdAt)}</td>
                  <td>{fmtTime(d.lastSeenAt)}</td>
                  <td>
                    {confirmId === d.id ? (
                      <span className="row">
                        <span>Forget {d.name}? It will need to pair again.</span>
                        <Button variant="danger" onClick={() => void revoke(d.id)}>
                          Forget
                        </Button>
                        <Button onClick={() => setConfirmId(null)}>Cancel</Button>
                      </span>
                    ) : (
                      <Button onClick={() => setConfirmId(d.id)}>Forget</Button>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </section>
    </div>
  );
}
