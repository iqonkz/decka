import { useCallback, useEffect, useMemo, useState } from "react";
import { t, type ConfigFile, type HttpCheck, type ValidationIssue } from "@decka/shared";
import { call } from "../ipc";
import Banner from "../ui/Banner";
import Button from "../ui/Button";
import Field from "../ui/Field";
import PhIcon from "../ui/PhIcon";
import { Group, PageHeader, Row, SectionHeader } from "../ui/SettingsGroup";
import { makeActionId } from "../editor/addButton";
import WeatherSection from "./WeatherSection";
import ObsSection from "./ObsSection";
import AiSection from "./AiSection";
import TwitchSection from "./TwitchSection";
import YoutubeSection from "./YoutubeSection";
import MailSection from "./MailSection";
import WinNotifSection from "./WinNotifSection";
import LyricsSection from "./LyricsSection";
import { lyricsGet, lyricsSet } from "./lyricsIpc";
import Switch from "../ui/Switch";
import {
  filterIntegrations,
  type IntegrationCategory,
  type IntegrationDef,
  type IntegrationFilterChip,
} from "./integrationsCatalog";

function CopyButton({ text }: { text: string }) {
  const [done, setDone] = useState(false);
  return (
    <Button
      onClick={() => {
        void navigator.clipboard.writeText(text).then(() => {
          setDone(true);
          window.setTimeout(() => setDone(false), 1500);
        });
      }}
    >
      {done ? t("common.copied") : t("common.copy")}
    </Button>
  );
}

function CommandBox({ title, command }: { title: string; command: string }) {
  return (
    <div className="command-box" style={{ marginBottom: 12 }}>
      <div className="row between">
        <strong>{title}</strong>
        <CopyButton text={command} />
      </div>
      <textarea className="mono" readOnly rows={command.split("\n").length} value={command} onFocus={(e) => e.currentTarget.select()} />
    </div>
  );
}

const newCheck = (n: number): HttpCheck => ({ id: `check-${n}`, label: "", url: "https://", intervalSec: 60, timeoutSec: 5 });

/** The ID of a check is made from its name and never typed. */
const checkId = (label: string, existing: HttpCheck[]): string => makeActionId(label || "check", Object.fromEntries(existing.map((c) => [c.id, 1])));

export default function IntegrationsTab() {
  const [config, setConfig] = useState<ConfigFile | null>(null);
  const [port, setPort] = useState(9247);
  const [showToken, setShowToken] = useState(false);
  const [confirmRegen, setConfirmRegen] = useState(false);
  const [checks, setChecks] = useState<HttpCheck[]>([]);
  const [editing, setEditing] = useState<{ index: number; check: HttpCheck } | null>(null);
  const [issues, setIssues] = useState<ValidationIssue[]>([]);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Catalog UI state
  const [activeIntegration, setActiveIntegration] = useState<string | null>(null);
  const [filterChip, setFilterChip] = useState<IntegrationFilterChip>("all");
  const [searchQuery, setSearchQuery] = useState("");
  const [lyricsEnabled, setLyricsEnabled] = useState(false);

  const load = useCallback(async () => {
    const c = await call("get_config");
    const s = await call("get_status");
    const lyr = await lyricsGet(c);
    setConfig(c);
    setChecks(c.collectors.checks);
    setPort(s.listening?.port ?? c.port);
    setLyricsEnabled(lyr);
  }, []);

  const toggleLyrics = useCallback(async (next: boolean) => {
    const prev = lyricsEnabled;
    setLyricsEnabled(next);
    try {
      await lyricsSet(next);
      setMessage(t("common.saved"));
    } catch (e) {
      setLyricsEnabled(prev);
      setError(String(e));
    }
  }, [lyricsEnabled]);

  useEffect(() => {
    load().catch((e) => setError(String(e)));
  }, [load]);

  const integrationsList: IntegrationDef[] = useMemo(() => {
    if (!config) return [];
    return [
      {
        id: "obs",
        name: "OBS Studio",
        titleKey: "integ.catalog_title",
        descKey: "integ.item_obs_desc",
        category: "streaming",
        icon: "broadcast",
        connected: Boolean(config.obs?.enabled),
        keywords: ["obs", "stream", "scenes", "recording", "broadcast"],
      },
      {
        id: "twitch",
        name: "Twitch",
        titleKey: "integ.catalog_title",
        descKey: "integ.item_twitch_desc",
        category: "streaming",
        icon: "chat-circle-text",
        connected: Boolean(config.twitch?.enabled),
        keywords: ["twitch", "stream", "chat", "live"],
      },
      {
        id: "youtube",
        name: "YouTube",
        titleKey: "integ.catalog_title",
        descKey: "integ.item_youtube_desc",
        category: "streaming",
        icon: "play-circle",
        connected: Boolean(config.youtube?.enabled),
        keywords: ["youtube", "video", "broadcast", "subscribers"],
      },
      {
        id: "mail",
        name: t("integ.item_mail_name"),
        titleKey: "integ.catalog_title",
        descKey: "integ.item_mail_desc",
        category: "notifications",
        icon: "envelope",
        connected: Boolean(config.mail?.accounts && config.mail.accounts.length > 0),
        keywords: ["email", "mail", "gmail", "imap", "unread"],
      },
      {
        id: "winnotif",
        name: t("winnotif.title"),
        titleKey: "winnotif.title",
        descKey: "integ.item_winnotif_desc",
        category: "notifications",
        icon: "bell",
        connected: Boolean(config.windowsNotifications?.enabled),
        keywords: ["windows", "notifications", "alerts", "toasts"],
      },
      {
        id: "weather",
        name: t("weather.title"),
        titleKey: "weather.title",
        descKey: "integ.item_weather_desc",
        category: "data",
        icon: "cloud-sun",
        connected: Boolean(config.weather?.enabled && config.weather?.place),
        keywords: ["weather", "temperature", "forecast", "city"],
      },
      {
        id: "checks",
        name: t("integ.checks"),
        titleKey: "integ.checks",
        descKey: "integ.item_checks_desc",
        category: "data",
        icon: "pulse",
        connected: Boolean(checks.length > 0),
        keywords: ["checks", "http", "uptime", "server", "ping"],
      },
      {
        id: "ai",
        name: t("integ.item_ai_name"),
        titleKey: "integ.catalog_title",
        descKey: "integ.item_ai_desc",
        category: "data",
        icon: "brain",
        connected: Boolean(config.ai?.claude?.enabled || config.ai?.codex?.enabled),
        keywords: ["ai", "claude", "codex", "llm", "limits"],
      },
      {
        id: "lyrics",
        name: t("integ.item_lyrics_name"),
        titleKey: "integ.item_lyrics_name",
        descKey: "integ.item_lyrics_desc",
        category: "data",
        icon: "music-notes",
        connected: lyricsEnabled,
        toggle: true,
        keywords: ["lyrics", "lrclib", "songs", "music", "synced", "текст", "песни"],
      },
      {
        id: "api_token",
        name: t("integ.api_token"),
        titleKey: "integ.api_token",
        descKey: "integ.item_api_desc",
        category: "developers",
        icon: "key",
        connected: Boolean(config.apiToken),
        keywords: ["api", "token", "auth", "developer", "key", "curl", "powershell", "examples", "webhook"],
      },
    ];
  }, [config, checks.length, lyricsEnabled]);

  const filteredIntegrations = useMemo(() => {
    return filterIntegrations(integrationsList, filterChip, searchQuery);
  }, [integrationsList, filterChip, searchQuery]);

  const connectedList = useMemo(() => {
    return integrationsList.filter((i) => i.connected);
  }, [integrationsList]);

  if (error) return <Banner kind="error">{error}</Banner>;
  if (!config) return <p>{t("common.loading")}</p>;

  const token = config.apiToken;
  const base = `http://127.0.0.1:${port}`;
  const auth = `-H "Authorization: Bearer ${token}"`;

  // JSON goes through a file: Windows PowerShell 5.1 mangles quotes in arguments of native programs.
  const curlJson = [
    `Set-Content -Path notify.json -Encoding ascii -Value '{"title":"Deploy finished","body":"web@1.4.2","level":"success","source":"ci"}'`,
    `curl.exe -s -X POST ${base}/api/notify ${auth} -H "Content-Type: application/json" --data-binary "@notify.json"`,
  ].join("\n");
  const curlText = `curl.exe -s -X POST ${base}/api/notify ${auth} -H "Content-Type: text/plain" -d "Backup finished"`;
  const powershell = `Invoke-RestMethod -Method Post -Uri ${base}/api/notify -Headers @{ Authorization = "Bearer ${token}" } -ContentType "application/json" -Body (@{ title = "Deploy finished"; level = "success" } | ConvertTo-Json)`;
  const curlStatus = [
    `Set-Content -Path status.json -Encoding ascii -Value '{"id":"my-job","label":"My job","state":"running","text":"working","group":"jobs","ttlSec":900}'`,
    `curl.exe -s -X POST ${base}/api/status ${auth} -H "Content-Type: application/json" --data-binary "@status.json"`,
  ].join("\n");

  async function regenerate() {
    setConfirmRegen(false);
    const result = await call("regenerate_api_token");
    setConfig({ ...config!, apiToken: result.apiToken });
  }

  async function saveChecks(next: HttpCheck[]) {
    const result = await call("save_config", { config: { ...config!, collectors: { ...config!.collectors, checks: next } } });
    if (result.ok) {
      setIssues([]);
      setMessage(t("common.saved"));
      await load();
    } else {
      setIssues(result.issues);
      setMessage(null);
    }
  }

  // If a detail section is opened
  if (activeIntegration !== null) {
    return (
      <div className="page-container">
        <div className="integrations-detail-header">
          <Button variant="secondary" onClick={() => setActiveIntegration(null)}>
            <span style={{ display: "inline-flex", alignItems: "center", gap: 6 }}>
              <PhIcon name="arrow-left" size={16} />
              {t("integ.back_to_catalog")}
            </span>
          </Button>
        </div>

        {activeIntegration === "obs" && <ObsSection config={config} onSaved={load} />}
        {activeIntegration === "twitch" && <TwitchSection config={config} onSaved={load} />}
        {activeIntegration === "youtube" && <YoutubeSection config={config} onSaved={load} />}
        {activeIntegration === "mail" && <MailSection config={config} onSaved={load} />}
        {activeIntegration === "winnotif" && <WinNotifSection config={config} onSaved={load} />}
        {activeIntegration === "weather" && <WeatherSection config={config} onSaved={load} />}
        {activeIntegration === "ai" && <AiSection config={config} onSaved={load} />}
        {activeIntegration === "lyrics" && <LyricsSection enabled={lyricsEnabled} onToggle={(v) => void toggleLyrics(v)} />}

        {activeIntegration === "api_token" && (
          <div>
            <PageHeader title={t("integ.api_token")} />
            <Group>
              <div style={{ padding: "14px 0" }}>
                <div className="row">
                  <input className="mono grow" readOnly type={showToken ? "text" : "password"} value={token} aria-label={t("integ.api_token")} />
                  <Button onClick={() => setShowToken(!showToken)}>{showToken ? t("integ.hide") : t("integ.show")}</Button>
                  <CopyButton text={token} />
                  <Button variant="danger" onClick={() => setConfirmRegen(true)}>
                    {t("integ.regenerate")}
                  </Button>
                </div>
                {confirmRegen && (
                  <div style={{ marginTop: 12 }}>
                    <Banner kind="warning">
                      <span>{t("integ.regenerate_warning")}</span>
                      <span className="row">
                        <Button variant="danger" onClick={() => void regenerate()}>
                          {t("integ.regenerate")}
                        </Button>
                        <Button onClick={() => setConfirmRegen(false)}>{t("common.cancel")}</Button>
                      </span>
                    </Banner>
                  </div>
                )}
                <div style={{ marginTop: 12 }}>
                  <Button variant="secondary" onClick={() => setActiveIntegration("examples")}>
                    {t("integ.show_examples")}
                  </Button>
                </div>
              </div>
            </Group>
          </div>
        )}

        {activeIntegration === "checks" && (
          <div>
            <PageHeader title={t("integ.checks")} subtitle={t("integ.checks_hint")} />
            <Group>
              <div style={{ padding: "14px 0" }}>
                {checks.length === 0 && <p className="muted">{t("integ.no_checks")}</p>}
                <ul className="plain-list" style={{ marginBottom: 12 }}>
                  {checks.map((c, i) => (
                    <li key={i} className="check-row" style={{ display: "flex", gap: 10, alignItems: "center", padding: "8px 0" }}>
                      <span className="status-dot wait" aria-hidden="true" />
                      <strong>{c.label}</strong>
                      <span className="muted grow">{c.url}</span>
                      <Button onClick={() => setEditing({ index: i, check: c })}>{t("integ.edit")}</Button>
                      <Button onClick={() => void saveChecks(checks.filter((_, n) => n !== i))}>{t("integ.remove")}</Button>
                    </li>
                  ))}
                </ul>
                {issues.length > 0 && (
                  <div style={{ marginBottom: 12 }}>
                    <Banner kind="error">
                      <ul>
                        {issues.map((iss, n) => (
                          <li key={n}>{iss.message}</li>
                        ))}
                      </ul>
                    </Banner>
                  </div>
                )}
                {message && (
                  <div style={{ marginBottom: 12 }}>
                    <Banner kind="success">{message}</Banner>
                  </div>
                )}
                <div className="row">
                  <Button variant="primary" disabled={checks.length >= 20} onClick={() => setEditing({ index: -1, check: newCheck(checks.length + 1) })}>
                    {t("integ.add_check")}
                  </Button>
                </div>
              </div>
            </Group>

            {editing && (
              <div className="dialog-backdrop">
                <div className="dialog" role="dialog" aria-label={t("integ.check")}>
                  <h2>{editing.index < 0 ? t("integ.add_check") : t("integ.edit_check")}</h2>
                  <Field label={t("integ.check_name")}>
                    <input autoFocus value={editing.check.label} onChange={(e) => setEditing({ ...editing, check: { ...editing.check, label: e.target.value } })} />
                  </Field>
                  <Field label={t("integ.check_address")}>
                    <input value={editing.check.url} onChange={(e) => setEditing({ ...editing, check: { ...editing.check, url: e.target.value } })} />
                  </Field>
                  <details className="advanced">
                    <summary>{t("common.more_options")}</summary>
                    <div className="grid-4">
                      <Field label={t("integ.check_every")}>
                        <input type="number" value={editing.check.intervalSec} onChange={(e) => setEditing({ ...editing, check: { ...editing.check, intervalSec: Number(e.target.value) } })} />
                      </Field>
                      <Field label={t("integ.give_up")}>
                        <input type="number" value={editing.check.timeoutSec} onChange={(e) => setEditing({ ...editing, check: { ...editing.check, timeoutSec: Number(e.target.value) } })} />
                      </Field>
                      <Field label={t("integ.expected")} hint={t("integ.expected_hint")}>
                        <input
                          type="number"
                          placeholder="200"
                          value={editing.check.expectStatus ?? ""}
                          onChange={(e) => setEditing({ ...editing, check: { ...editing.check, expectStatus: e.target.value === "" ? undefined : Number(e.target.value) } })}
                        />
                      </Field>
                      <Field label={t("integ.group")}>
                        <input value={editing.check.group ?? ""} onChange={(e) => setEditing({ ...editing, check: { ...editing.check, group: e.target.value || undefined } })} />
                      </Field>
                    </div>
                  </details>
                  <div className="row">
                    <Button
                      variant="primary"
                      onClick={() => {
                        const next = editing.index < 0 ? [...checks, { ...editing.check, id: checkId(editing.check.label, checks) }] : checks.map((c, n) => (n === editing.index ? editing.check : c));
                        setEditing(null);
                        void saveChecks(next);
                      }}
                    >
                      {t("common.save")}
                    </Button>
                    <Button onClick={() => setEditing(null)}>{t("common.cancel")}</Button>
                  </div>
                </div>
              </div>
            )}
          </div>
        )}

        {activeIntegration === "examples" && (
          <div>
            <PageHeader title={t("integ.show_examples")} />
            <SectionHeader title={t("integ.send_notification")} />
            <Group>
              <div style={{ padding: "14px 0" }}>
                <CommandBox title="curl.exe (JSON)" command={curlJson} />
                <CommandBox title={t("integ.curl_plain")} command={curlText} />
                <CommandBox title="PowerShell" command={powershell} />
                <div>
                  <Button onClick={() => void call("send_test_notification")}>{t("integ.send_test")}</Button>
                </div>
              </div>
            </Group>

            <SectionHeader title={t("integ.update_status")} />
            <Group>
              <div style={{ padding: "14px 0" }}>
                <CommandBox title="curl.exe" command={curlStatus} />
              </div>
            </Group>
          </div>
        )}
      </div>
    );
  }

  // Catalog View
  return (
    <div className="page-container">
      <PageHeader icon="plugs-connected" title={t("integ.catalog_title")} />

      {/* Top Status Bar: up to 4 overlapping connected icons and N connected with caret-right */}
      <button
        type="button"
        className="integrations-status-bar"
        onClick={() => setFilterChip("connected")}
        title={t("integ.filter_connected")}
      >
        <div className="integrations-overlapping-icons">
          {connectedList.slice(0, 4).map((item) => (
            <span key={item.id} className="integrations-overlap-icon">
              <PhIcon name={item.icon} size={14} />
            </span>
          ))}
        </div>
        <span>{t("integ.connected_count", { n: connectedList.length })}</span>
        <PhIcon name="caret-right" size={14} />
      </button>

      {/* Search Bar */}
      <div className="integrations-search">
        <PhIcon name="magnifying-glass" size={16} />
        <input
          type="search"
          placeholder={t("sidebar.search")}
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
        />
      </div>

      {/* Filter Chips */}
      <div className="integrations-chips" role="radiogroup">
        {(
          [
            { id: "all", label: t("integ.filter_all") },
            { id: "connected", label: t("integ.filter_connected") },
            { id: "streaming", label: t("integ.filter_streaming") },
            { id: "notifications", label: t("integ.filter_notifications") },
            { id: "data", label: t("integ.filter_data") },
            { id: "developers", label: t("integ.filter_developers") },
          ] as const
        ).map((chip) => (
          <button
            key={chip.id}
            type="button"
            className={`integrations-chip${filterChip === chip.id ? " active" : ""}`}
            onClick={() => setFilterChip(chip.id)}
          >
            {chip.label}
          </button>
        ))}
      </div>

      {/* Cards Grid: 2 columns, gap 12 */}
      <div className="integrations-grid">
        {filteredIntegrations.map((item) => {
          const desc = t(item.descKey);
          return (
            <div
              key={item.id}
              id={`section-integrations-${item.id}`}
              className="integration-card"
              onClick={() => setActiveIntegration(item.id)}
              role="button"
              tabIndex={0}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  setActiveIntegration(item.id);
                }
              }}
            >
              <div className="integration-badge">
                <PhIcon name={item.icon} size={22} />
              </div>
              <div className="integration-card-content">
                <div className="integration-card-name">{item.name}</div>
                <div className="integration-card-desc" title={desc}>
                  {desc}
                </div>
              </div>
              <div className="integration-card-action">
                {item.toggle ? (
                  <span onClick={(e) => e.stopPropagation()} onKeyDown={(e) => e.stopPropagation()}>
                    <Switch
                      checked={item.connected}
                      onChange={(next) => void toggleLyrics(next)}
                      aria-label={item.name}
                    />
                  </span>
                ) : item.connected ? (
                  <span className="integration-connected-badge">
                    <span className="check-icon">
                      <PhIcon name="check" size={14} />
                    </span>
                    {t("integ.connected")}
                  </span>
                ) : (
                  <Button variant="primary" onClick={(e) => {
                    e.stopPropagation();
                    setActiveIntegration(item.id);
                  }}>
                    {t("integ.set_up")}
                  </Button>
                )}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
