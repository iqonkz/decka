import { useEffect, useRef, useState } from "react";
import type { AgentStatus } from "@decka/shared";
import { call, onStatus } from "../ipc";
import Banner from "../ui/Banner";
import Button from "../ui/Button";
import PairingTab from "./PairingTab";

/** The firewall hint appears only when a phone has been paired and, two minutes later, none has connected. */
export const FIREWALL_WAIT_MS = 120_000;

/** `Connected · Galaxy A57 · USB`, `Waiting for your phone` or `Server stopped` (DESIGN.md 4.6a). */
export function phoneSummary(status: AgentStatus): { dot: "on" | "wait" | "off"; text: string } {
  if (!status.listening) return { dot: "off", text: "Server stopped" };
  const client = status.clients[0];
  if (!client) return { dot: "wait", text: "Waiting for your phone" };
  const usb = client.remote.startsWith("127.0.0.1") || client.remote.startsWith("[::1]");
  return { dot: "on", text: `Connected · ${client.name} · ${usb ? "USB" : "Wi-Fi"}` };
}

/** Status and pairing on one page: one big card on top, the technical details under `More options`. */
export default function PhoneTab() {
  const [status, setStatus] = useState<AgentStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [paired, setPaired] = useState(0);
  const [waited, setWaited] = useState(false);
  const everConnected = useRef(false);

  useEffect(() => {
    let off: (() => void) | null = null;
    let disposed = false;
    call("get_status").then(setStatus, (e) => setError(String(e)));
    call("list_devices").then((d) => setPaired(d.length), () => {});
    onStatus(setStatus).then((unlisten) => {
      if (disposed) unlisten();
      else off = unlisten;
    });
    const timer = window.setTimeout(() => setWaited(true), FIREWALL_WAIT_MS);
    return () => {
      disposed = true;
      off?.();
      window.clearTimeout(timer);
    };
  }, []);

  if (error) return <Banner kind="error">{error}</Banner>;
  if (!status) return <p>Loading…</p>;
  if (status.clients.length > 0) everConnected.current = true;

  const port = status.listening?.port ?? 9247;
  const summary = phoneSummary(status);
  const firewallCommand = `netsh advfirewall firewall add rule name="Decka" dir=in action=allow protocol=TCP localport=${port} profile=private`;
  const firewallProblem = status.listening?.bind === "0.0.0.0" && paired > 0 && waited && !everConnected.current;

  return (
    <div className="stack">
      <h2>Phone</h2>

      <section className="card phone-card">
        <span className={`status-dot ${summary.dot}`} aria-hidden="true" />
        <strong>{summary.text}</strong>
        {!status.listening && (
          <>
            <span className="muted">{status.listenError ?? ""}</span>
            <Button onClick={() => call("restart_server").then(setStatus, (e) => setError(String(e)))}>Restart</Button>
          </>
        )}
      </section>

      <PairingTab embedded />

      {firewallProblem && (
        <section className="card">
          <h3>The phone cannot connect over Wi-Fi?</h3>
          <p>Allow Decka in Windows Firewall (private networks) or run this command as administrator:</p>
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

      <details className="advanced">
        <summary>More options</summary>
        <section className="card">
          <h3>Addresses</h3>
          {status.listening && (
            <p className="muted">
              Listening on {status.listening.bind}:{status.listening.port}
            </p>
          )}
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
          <h3>USB</h3>
          {!status.usb.enabled ? (
            <p className="muted">Off</p>
          ) : !status.usb.adbFound ? (
            <p className="warn-text">adb not found{status.usb.lastError ? `: ${status.usb.lastError}` : ""}</p>
          ) : status.usb.devices.length === 0 ? (
            <p className="muted">No phones plugged in</p>
          ) : (
            <ul>
              {status.usb.devices.map((d) => (
                <li key={d.serial}>
                  {d.serial} {d.reversed && <span className="tag">ready</span>}
                </li>
              ))}
            </ul>
          )}
        </section>
        <section className="card">
          <h3>Active app</h3>
          <p>{status.focusApp ?? "—"}</p>
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
      </details>
    </div>
  );
}