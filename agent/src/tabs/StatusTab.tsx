import { useEffect, useState } from "react";
import type { AgentStatus } from "@decka/shared";
import { call, onStatus } from "../ipc";
import Banner from "../ui/Banner";
import Button from "../ui/Button";

export default function StatusTab() {
  const [status, setStatus] = useState<AgentStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    let off: (() => void) | null = null;
    let disposed = false;
    call("get_status").then(setStatus, (e) => setError(String(e)));
    onStatus(setStatus).then((unlisten) => {
      if (disposed) unlisten();
      else off = unlisten;
    });
    return () => {
      disposed = true;
      off?.();
    };
  }, []);

  if (error) return <Banner kind="error">{error}</Banner>;
  if (!status) return <p>Loading…</p>;

  const port = status.listening?.port ?? 9247;
  const firewallCommand = `netsh advfirewall firewall add rule name="Decka" dir=in action=allow protocol=TCP localport=${port} profile=private`;

  return (
    <div className="stack">
      <h2>Status</h2>

      <section className="card">
        <h3>Server</h3>
        {status.listening ? (
          <p className="ok-text">
            Listening on {status.listening.bind}:{status.listening.port}
          </p>
        ) : (
          <Banner kind="error">
            <div>{status.listenError ?? "The server is not running."}</div>
            <Button onClick={() => call("restart_server").then(setStatus, (e) => setError(String(e)))}>Retry</Button>
          </Banner>
        )}
      </section>

      <section className="card">
        <h3>Addresses</h3>
        {status.lanIps.length === 0 ? (
          <p className="muted">No local network address found.</p>
        ) : (
          <ul>
            {status.lanIps.map((ip) => (
              <li key={ip}>
                <code>{`http://${ip}:${port}`}</code>
              </li>
            ))}
          </ul>
        )}
      </section>

      <section className="card">
        <h3>Connected devices</h3>
        {status.clients.length === 0 ? (
          <p className="muted">No devices connected</p>
        ) : (
          <table>
            <thead>
              <tr>
                <th>Name</th>
                <th>Remote</th>
                <th>Connected since</th>
              </tr>
            </thead>
            <tbody>
              {status.clients.map((c, i) => (
                <tr key={`${c.deviceId}-${i}`}>
                  <td>{c.name}</td>
                  <td>{c.remote}</td>
                  <td>{new Date(c.connectedAt).toLocaleTimeString()}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </section>

      <section className="card">
        <h3>USB</h3>
        {!status.usb.enabled ? (
          <p className="muted">Disabled</p>
        ) : !status.usb.adbFound ? (
          <p className="warn-text">adb not found{status.usb.lastError ? `: ${status.usb.lastError}` : ""}</p>
        ) : status.usb.devices.length === 0 ? (
          <p className="muted">No devices</p>
        ) : (
          <ul>
            {status.usb.devices.map((d) => (
              <li key={d.serial}>
                {d.serial} {d.reversed && <span className="tag">forwarded</span>}
              </li>
            ))}
          </ul>
        )}
      </section>

      <section className="card">
        <h3>Active app</h3>
        <p>Active app: {status.focusApp ?? "—"}</p>
      </section>

      <section className="card">
        <h3>Config folder</h3>
        <p>
          <code>{status.configDir}</code>
        </p>
        <div className="row">
          <Button onClick={() => call("open_config_dir")}>Open</Button>
          <Button onClick={() => call("open_logs_dir")}>Open logs</Button>
        </div>
      </section>

      {status.listening?.bind === "0.0.0.0" && (
        <section className="card">
          <h3>Windows Firewall</h3>
          <p>If the phone can't connect over Wi-Fi, allow Decka in Windows Firewall (private networks) or run this command as administrator:</p>
          <div className="row">
            <input className="mono" readOnly value={firewallCommand} onFocus={(e) => e.currentTarget.select()} />
            <Button
              onClick={() => {
                void navigator.clipboard.writeText(firewallCommand).then(() => {
                  setCopied(true);
                  window.setTimeout(() => setCopied(false), 1500);
                });
              }}
            >
              {copied ? "Copied" : "Copy"}
            </Button>
          </div>
        </section>
      )}
    </div>
  );
}
