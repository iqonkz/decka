import { useEffect, useState } from "react";
import { t, type HaStatus } from "@decka/shared";
import { call } from "../ipc";
import Banner from "../ui/Banner";
import Button from "../ui/Button";
import Field from "../ui/Field";
import { Group, PageHeader, Row, SectionHeader } from "../ui/SettingsGroup";
import { saveHa } from "./m30Ipc";

export interface HaSectionProps {
  /** Tests give the answer of `ha_get`. */
  initialStatus?: HaStatus;
  onSaved?: () => void;
}

/** T30.1b: Home Assistant integration settings (URL, token, the devices chosen for voice and buttons). */
export default function HaSection({ initialStatus, onSaved }: HaSectionProps = {}) {
  const [status, setStatus] = useState<HaStatus | null>(initialStatus ?? null);
  const [url, setUrl] = useState(initialStatus?.url ?? "");
  const [token, setToken] = useState("");
  const [entities, setEntities] = useState<HaStatus["entities"]>(initialStatus?.entities ?? []);
  const [query, setQuery] = useState("");
  const [message, setMessage] = useState<{ kind: "success" | "error"; text: string } | null>(null);
  const [busy, setBusy] = useState(false);

  function show(next: HaStatus) {
    setStatus(next);
    setUrl(next.url);
    setEntities(next.entities);
  }

  const failed = (e: unknown) => setMessage({ kind: "error", text: String(e) });

  useEffect(() => {
    if (initialStatus) return;
    let active = true;
    call("ha_get").then(
      (s) => active && show(s),
      (e) => active && failed(e),
    );
    return () => {
      active = false;
    };
  }, [initialStatus]);

  async function run(work: () => Promise<void>) {
    setBusy(true);
    setMessage(null);
    try {
      await work();
    } catch (e) {
      failed(e);
    } finally {
      setBusy(false);
    }
  }

  const save = () =>
    run(async () => {
      show(await saveHa(call, url, token, entities));
      setToken("");
      setMessage({ kind: "success", text: t("common.saved") });
      onSaved?.();
    });

  const test = () =>
    run(async () => {
      const n = await call("ha_test", { url: url.trim(), token: token.trim() });
      setMessage({ kind: "success", text: t("ha.connected_count", { n }) });
    });

  const remove = () =>
    run(async () => {
      show(await saveHa(call, "", "", []));
      setToken("");
      onSaved?.();
    });

  const q = query.trim().toLowerCase();
  const shown = q ? entities.filter((e) => e.id.includes(q) || e.name.toLowerCase().includes(q)) : entities;

  return (
    <div>
      <PageHeader icon="house" title={t("m30.ha_name")} subtitle={t("m30.ha_desc")} />
      {message && <Banner kind={message.kind}>{message.text}</Banner>}
      {!message && status?.error && <Banner kind="error">{status.error}</Banner>}

      <Group>
        <div style={{ padding: "12px 16px", display: "flex", flexDirection: "column", gap: 12 }}>
          <Field label={t("m30.ha_url")}>
            <input type="url" value={url} onChange={(e) => setUrl(e.target.value)} placeholder="http://homeassistant.local:8123" />
          </Field>
          <Field label={t("m30.ha_token")}>
            <input type="password" value={token} onChange={(e) => setToken(e.target.value)} placeholder={status?.hasToken ? t("ha.token_saved") : ""} />
          </Field>
          <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
            <Button variant="primary" disabled={busy || url.trim() === ""} onClick={() => void save()}>
              {t("m30.ha_save")}
            </Button>
            <Button disabled={busy || url.trim() === ""} onClick={() => void test()}>
              {t("m30.ha_test")}
            </Button>
            {status?.hasToken && (
              <Button variant="danger" disabled={busy} onClick={() => void remove()}>
                {t("ha.remove")}
              </Button>
            )}
          </div>
        </div>
      </Group>

      <SectionHeader title={t("m30.ha_entities")} />
      {entities.length === 0 ? (
        <p className="muted">{t("ha.empty")}</p>
      ) : (
        <>
          <p className="muted">{t("ha.choose_hint")}</p>
          <input type="search" aria-label={t("ha.search")} placeholder={t("ha.search")} value={query} onChange={(e) => setQuery(e.target.value)} />
          <Group>
            {shown.map((ent) => (
              <Row key={ent.id} title={ent.name} description={ent.id}>
                <input
                  type="checkbox"
                  checked={ent.enabled}
                  onChange={() => setEntities((prev) => prev.map((e) => (e.id === ent.id ? { ...e, enabled: !e.enabled } : e)))}
                  aria-label={ent.name}
                />
              </Row>
            ))}
          </Group>
        </>
      )}
    </div>
  );
}
