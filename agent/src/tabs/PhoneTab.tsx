import { useEffect, useRef, useState } from "react";
import { t, type AgentStatus } from "@decka/shared";
import { call, onStatus } from "../ipc";
import Banner from "../ui/Banner";
import Button from "../ui/Button";
import DiagSection from "./DiagSection";
import NetworkLine from "./NetworkLine";
import PairingTab from "./PairingTab";
import { Group, PageHeader, Row, SectionHeader } from "../ui/SettingsGroup";

/** The firewall hint appears when, a minute after this page was opened, no phone has connected (also when the first pairing fails). */
export const FIREWALL_WAIT_MS = 60_000;

/** `Connected · Galaxy A57 · USB`, `Waiting for your phone` or `Server stopped` (DESIGN.md 4.6a). */
export function phoneSummary(status: AgentStatus): { dot: "on" | "wait" | "off"; text: string } {
  if (!status.listening) return { dot: "off", text: t("phone.server_stopped") };
  const client = status.clients[0];
  if (!client) return { dot: "wait", text: t("phone.waiting") };
  const usb = client.remote.startsWith("127.0.0.1") || client.remote.startsWith("[::1]");
  return { dot: "on", text: `${t("phone.connected")} · ${client.name} · ${usb ? "USB" : "Wi-Fi"}` };
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
    call("list_devices").then((d) => setPaired(d ? d.length : 0), () => {});
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
  if (!status) return <p>{t("common.loading")}</p>;
  if (status.clients.length > 0) everConnected.current = true;

  const port = status.listening?.port ?? 9247;
  const summary = phoneSummary(status);
  const firewallCommand = `netsh advfirewall firewall add rule name="Decka" dir=in action=allow protocol=TCP localport=${port} profile=private,public`;
  // A hotspot (of the PC or of a phone) is a "public" network for Windows, so the rule covers both kinds of network.
  const firewallProblem = status.listening?.bind === "0.0.0.0" && waited && !everConnected.current;

  return (
    <div className="page-container">
      <PageHeader icon="device-mobile" title={t("tab.phone")} />

      <Group id="phone-status">
        <div className="phone-card" style={{ padding: "14px 0", display: "flex", gap: 12, alignItems: "center", flexWrap: "wrap" }}>
          <span className={`status-dot ${summary.dot}`} aria-hidden="true" />
          <strong>{summary.text}</strong>
          {!status.listening && (
            <>
              <span className="muted">
                {status.listenBusy
                  ? status.listenBusy.app
                    ? t("phone.port_busy", { port: status.listenBusy.port, app: status.listenBusy.app })
                    : t("phone.port_busy_unknown", { port: status.listenBusy.port })
                  : (status.listenError ?? "")}
              </span>
              <Button onClick={() => call("restart_server").then(setStatus, (e) => setError(String(e)))}>
                {status.listenBusy ? t("link.try_again") : t("phone.restart")}
              </Button>
            </>
          )}
        </div>
      </Group>

      <NetworkLine />

      <div id="phone-pairing" style={{ marginTop: 24 }}>
        <PairingTab embedded />
      </div>

      <div id="phone-diag" style={{ marginTop: 24 }}>
        <DiagSection />
      </div>

      {firewallProblem && (
        <div id="phone-firewall" style={{ marginTop: 24 }}>
          <SectionHeader title={t("phone.firewall_title")} />
          <Group>
            <div style={{ padding: "14px 0" }}>
              <p>{t("phone.firewall_text")}</p>
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
                  {copied ? t("common.copied") : t("common.copy")}
                </Button>
              </div>
            </div>
          </Group>
        </div>
      )}

      <details className="advanced" style={{ marginTop: 32 }}>
        <summary style={{ cursor: "pointer", fontWeight: 600 }}>{t("common.more_options")}</summary>
        <div style={{ marginTop: 12 }}>
          <SectionHeader title={t("phone.addresses")} />
          <Group>
            <div style={{ padding: "14px 0" }}>
              {status.listening && (
                <p className="muted" style={{ margin: "0 0 8px" }}>
                  {t("phone.listening", { where: `${status.listening.bind}:${status.listening.port}` })}
                </p>
              )}
              {status.lanIps.length === 0 ? (
                <p className="muted" style={{ margin: 0 }}>{t("phone.no_address")}</p>
              ) : (
                <ul style={{ margin: 0, paddingInlineStart: 20 }}>
                  {status.lanIps.map((ip) => (
                    <li key={ip}>
                      <code>{`http://${ip}:${port}`}</code>
                    </li>
                  ))}
                </ul>
              )}
            </div>
          </Group>

          <SectionHeader title="USB" />
          <Group>
            <div style={{ padding: "14px 0" }}>
              {!status.usb.enabled ? (
                <p className="muted" style={{ margin: 0 }}>{t("phone.usb_off")}</p>
              ) : !status.usb.adbFound ? (
                <p className="warn-text" style={{ margin: 0 }}>{t("phone.adb_not_found")}{status.usb.lastError ? `: ${status.usb.lastError}` : ""}</p>
              ) : status.usb.devices.length === 0 ? (
                <p className="muted" style={{ margin: 0 }}>{t("phone.no_usb_phones")}</p>
              ) : (
                <ul style={{ margin: 0, paddingInlineStart: 20 }}>
                  {status.usb.devices.map((d) => (
                    <li key={d.serial}>
                      {d.serial} {d.reversed && <span className="tag">{t("phone.usb_ready")}</span>}
                    </li>
                  ))}
                </ul>
              )}
            </div>
          </Group>

          <SectionHeader title={t("phone.active_app")} />
          <Group>
            <Row title={t("phone.active_app")}>
              <span>{status.focusApp ?? "—"}</span>
            </Row>
          </Group>

          <SectionHeader title={t("phone.config_folder")} />
          <Group>
            <Row title={<code>{status.configDir}</code>}>
              <div className="row">
                <Button onClick={() => call("open_config_dir")}>{t("common.open")}</Button>
                <Button onClick={() => call("open_logs_dir")}>{t("phone.open_logs")}</Button>
              </div>
            </Row>
          </Group>
        </div>
      </details>
    </div>
  );
}
