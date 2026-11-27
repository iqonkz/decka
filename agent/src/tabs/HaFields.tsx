import { t, type HaService, type HaStatus } from "@decka/shared";
import { useEffect, useState } from "react";
import { call } from "../ipc";
import Banner from "../ui/Banner";
import Field from "../ui/Field";

const SERVICES = ["toggle", "turn_on", "turn_off"] as const;

export interface HaFieldsProps {
  entity: string;
  service: HaService;
  onChange: (entity: string, service: HaService) => void;
  initialStatus?: HaStatus;
}

/** T30.8: the entity and the service of a `ha` button. The entities come from Home Assistant; an id can be typed too. */
export default function HaFields({ entity, service, onChange, initialStatus }: HaFieldsProps) {
  const [status, setStatus] = useState<HaStatus | null>(initialStatus ?? null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (initialStatus) return;
    let active = true;
    call("ha_get").then(
      (s) => active && setStatus(s),
      (e) => active && setError(String(e)),
    );
    return () => {
      active = false;
    };
  }, [initialStatus]);

  const scene = entity.startsWith("scene.");
  const problem = error ?? status?.error ?? (status && !status.hasToken ? t("ha.not_set") : null);

  return (
    <>
      {problem && <Banner kind="error">{problem}</Banner>}
      <Field label={t("ha.entity")}>
        <input list="ha-entities" value={entity} onChange={(e) => onChange(e.target.value, service)} placeholder="light.bedroom" />
      </Field>
      <datalist id="ha-entities">
        {status?.entities.map((e) => (
          <option key={e.id} value={e.id}>
            {e.name}
          </option>
        ))}
      </datalist>
      {!scene && (
        <div className="tile-grid" role="group" aria-label={t("ha.service")}>
          {SERVICES.map((s) => (
            <button key={s} type="button" className={service === s ? "on" : ""} aria-pressed={service === s} onClick={() => onChange(entity, s)}>
              {t(`ha.service_${s}`)}
            </button>
          ))}
        </div>
      )}
    </>
  );
}
