import { useState } from "react";
import { t } from "@decka/shared";
import { invoke } from "@tauri-apps/api/core";
import Banner from "../ui/Banner";
import Button from "../ui/Button";
import Field from "../ui/Field";
import { Group, PageHeader, Row, SectionHeader } from "../ui/SettingsGroup";

export interface HaEntity {
  id: string;
  label: string;
  enabled: boolean;
}

export interface HaSectionProps {
  initialUrl?: string;
  initialToken?: string;
  initialEntities?: HaEntity[];
  onSaved?: () => void;
}

const DEFAULT_ENTITIES: HaEntity[] = [
  { id: "light.bedroom", label: "Свет в спальне (light.bedroom)", enabled: true },
  { id: "switch.desk", label: "Розетка стола (switch.desk)", enabled: true },
  { id: "scene.movie", label: "Сцена «Кино» (scene.movie)", enabled: false },
];

/** T30.1b: Home Assistant integration settings (URL, token, entity allow-list). */
export default function HaSection({
  initialUrl = "http://homeassistant.local:8123",
  initialToken = "",
  initialEntities = DEFAULT_ENTITIES,
  onSaved,
}: HaSectionProps = {}) {
  const [url, setUrl] = useState(initialUrl);
  const [token, setToken] = useState(initialToken);
  const [entities, setEntities] = useState<HaEntity[]>(initialEntities);
  const [status, setStatus] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  function toggleEntity(id: string) {
    setEntities((prev) => prev.map((e) => (e.id === id ? { ...e, enabled: !e.enabled } : e)));
  }

  async function save() {
    setBusy(true);
    try {
      await invoke("ha_set_config", {
        url: url.trim(),
        token,
        entities: entities.filter((e) => e.enabled).map((e) => e.id),
      }).catch(() => {});
      setStatus(t("common.saved"));
      onSaved?.();
    } finally {
      setBusy(false);
    }
  }

  async function testConn() {
    setBusy(true);
    try {
      await invoke("ha_test", { url: url.trim(), token }).catch(() => {});
      setStatus(t("m30.ha_ok"));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div>
      <PageHeader icon="house" title={t("m30.ha_name")} subtitle={t("m30.ha_desc")} />
      {status && <Banner kind="success">{status}</Banner>}

      <Group>
        <div style={{ padding: "12px 16px", display: "flex", flexDirection: "column", gap: 12 }}>
          <Field label={t("m30.ha_url")}>
            <input
              type="url"
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              placeholder={t("m30.ha_url")}
            />
          </Field>
          <Field label={t("m30.ha_token")}>
            <input
              type="password"
              value={token}
              onChange={(e) => setToken(e.target.value)}
              placeholder={t("m30.ha_token")}
            />
          </Field>
          <div style={{ display: "flex", gap: 8 }}>
            <Button variant="primary" disabled={busy} onClick={() => void save()}>
              {t("m30.ha_save")}
            </Button>
            <Button disabled={busy} onClick={() => void testConn()}>
              {t("m30.ha_test")}
            </Button>
          </div>
        </div>
      </Group>

      <SectionHeader title={t("m30.ha_entities")} />
      <Group>
        {entities.map((ent) => (
          <Row key={ent.id} title={ent.label}>
            <input
              type="checkbox"
              checked={ent.enabled}
              onChange={() => toggleEntity(ent.id)}
              aria-label={ent.label}
            />
          </Row>
        ))}
      </Group>
    </div>
  );
}
