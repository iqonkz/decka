import { t, type Action, type HaService, type HttpMethod, type MediaOp, type ShotArea, type SystemOp } from "@decka/shared";
import { useState } from "react";
import Disclosure from "../ui/Disclosure";
import Field from "../ui/Field";
import Switch from "../ui/Switch";
import { presetActions } from "../editor/addButton";
import ChordField from "../editor/ChordField";
import QuickPicker from "../editor/QuickPicker";
import PhIcon from "../ui/PhIcon";
import HaFields from "./HaFields";
import { formatKeys, parseKeys } from "./keys";
import { applyInstalledApp, applyPickedProgram } from "./programPick";

export type ActionType = Action["type"];

export const MEDIA_OPS: MediaOp[] = ["play_pause", "next", "prev", "vol_up", "vol_down", "mute"];
export const SHOT_AREAS: ShotArea[] = ["screen", "window", "region"];
const SHOT_ICONS: Record<ShotArea, string> = { screen: "monitor", window: "app-window", region: "selection" };
/** The kinds of action with their labels in the current language. */
export const actionTypes = (): { id: ActionType; label: string; icon: string }[] => [
  { id: "launch", label: t("form.type_app"), icon: "rocket-launch" },
  { id: "open_url", label: t("form.type_website"), icon: "globe" },
  { id: "hotkey", label: t("form.type_shortcut"), icon: "keyboard" },
  { id: "media", label: t("form.type_media"), icon: "play-circle" },
  { id: "system", label: t("form.type_system"), icon: "power" },
  { id: "screenshot", label: t("m29.type_screenshot"), icon: "camera" },
  { id: "http", label: t("m29.type_webhook"), icon: "link" },
  { id: "notes_record", label: t("notes.action_type"), icon: "microphone" },
  { id: "ha", label: t("ha.action_type"), icon: "house" },
];

/** T29.P: a header value kept by the agent under DPAPI shows as this; a line left so keeps the stored value. */
export const HEADER_MASK = "••••••";
const STORED_PREFIX = "dpapi:";

/** `Name: value` lines -> headers; `null` when a line has no name and colon. */
export function parseHeaders(text: string): Record<string, string> | null {
  const out: Record<string, string> = {};
  for (const line of text.split(/\r?\n/).map((l) => l.trim()).filter((l) => l !== "")) {
    const colon = line.indexOf(":");
    if (colon <= 0) return null;
    out[line.slice(0, colon).trim()] = line.slice(colon + 1).trim();
  }
  return out;
}

/** Everything the form edits, as strings and booleans. */
export interface Draft {
  id: string;
  name: string;
  type: ActionType;
  keysText: string;
  mediaOp: MediaOp;
  systemOp: SystemOp;
  program: string;
  argsText: string;
  cwd: string;
  hidden: boolean;
  url: string;
  shotArea: ShotArea;
  method: HttpMethod;
  body: string;
  headersText: string;
  /** The protected values of the action being edited, by header name. */
  headersStored: Record<string, string>;
  haEntity: string;
  haService: HaService;
  toggleVar: string;
}

export const emptyDraft = (): Draft => ({
  id: "",
  name: "",
  type: "hotkey",
  keysText: "",
  mediaOp: "play_pause",
  systemOp: "lock",
  program: "",
  argsText: "",
  cwd: "",
  hidden: false,
  url: "",
  shotArea: "screen",
  method: "GET",
  body: "",
  headersText: "",
  headersStored: {},
  haEntity: "",
  haService: "toggle",
  toggleVar: "",
});

export function toDraft(id: string, action: Action): Draft {
  const draft: Draft = { ...emptyDraft(), id, name: action.name, type: action.type, toggleVar: action.toggleVar ?? "" };
  switch (action.type) {
    case "hotkey":
      draft.keysText = formatKeys(action.keys);
      break;
    case "media":
      draft.mediaOp = action.op;
      break;
    case "launch":
      draft.program = action.program;
      draft.argsText = (action.args ?? []).join("\n");
      draft.cwd = action.cwd ?? "";
      draft.hidden = action.hidden === true;
      break;
    case "open_url":
      draft.url = action.url;
      break;
    case "system":
      draft.systemOp = action.op;
      break;
    case "screenshot":
      draft.shotArea = action.area;
      break;
    case "http":
      draft.url = action.url;
      draft.method = action.method;
      draft.body = action.body ?? "";
      draft.headersStored = Object.fromEntries(Object.entries(action.headers ?? {}).filter(([, value]) => value.startsWith(STORED_PREFIX)));
      draft.headersText = Object.entries(action.headers ?? {})
        .map(([name, value]) => `${name}: ${value.startsWith(STORED_PREFIX) ? HEADER_MASK : value}`)
        .join("\n");
      break;
    case "notes_record":
      break; // nothing to fill in
    case "ha":
      draft.haEntity = action.entityId;
      draft.haService = action.service;
      break;
    case "audio":
    case "obs":
    case "twitch":
    case "youtube":
    case "sequence":
      break; // edited by the library and the sequence editor (T15.4), not by this form
  }
  return draft;
}

/** Draft -> Action. Server-side validation still runs on Save; this only builds the shape. */
export function fromDraft(d: Draft): { action: Action } | { error: string } {
  const base = { name: d.name.trim(), ...(d.toggleVar.trim() ? { toggleVar: d.toggleVar.trim() } : {}) };
  switch (d.type) {
    case "hotkey": {
      const keys = parseKeys(d.keysText);
      if (!keys) return { error: t("form.keys_error") };
      return { action: { ...base, type: "hotkey", keys } };
    }
    case "media":
      return { action: { ...base, type: "media", op: d.mediaOp } };
    case "launch": {
      const args = d.argsText.split(/\r?\n/).filter((line) => line !== "");
      return {
        action: {
          ...base,
          type: "launch",
          program: d.program.trim(),
          ...(args.length > 0 ? { args } : {}),
          ...(d.cwd.trim() ? { cwd: d.cwd.trim() } : {}),
          ...(d.hidden ? { hidden: true } : {}),
        },
      };
    }
    case "open_url":
      return { action: { ...base, type: "open_url", url: d.url.trim() } };
    case "system":
      return { action: { ...base, type: "system", op: d.systemOp } };
    case "screenshot":
      return { action: { ...base, type: "screenshot", area: d.shotArea } };
    case "notes_record":
      return { action: { ...base, type: "notes_record" } };
    case "ha": {
      const entityId = d.haEntity.trim().toLowerCase();
      if (!/^[a-z0-9_]+\.[a-z0-9_]+$/.test(entityId) || entityId.length > 61) return { error: t("ha.entity_error") };
      // A scene is only ever started.
      const service: HaService = entityId.startsWith("scene.") ? "scene" : d.haService === "scene" ? "toggle" : d.haService;
      return { action: { ...base, type: "ha", entityId, service } };
    }
    case "http": {
      const headers = parseHeaders(d.headersText);
      if (!headers) return { error: t("m29.webhook_headers_error") };
      for (const [name, value] of Object.entries(headers)) {
        if (value !== HEADER_MASK) continue;
        const stored = d.headersStored[name];
        if (stored === undefined) return { error: t("m29.webhook_headers_again", { name }) };
        headers[name] = stored;
      }
      return {
        action: {
          ...base,
          type: "http",
          method: d.method,
          url: d.url.trim(),
          ...(d.method === "POST" && d.body !== "" ? { body: d.body } : {}),
          ...(Object.keys(headers).length > 0 ? { headers } : {}),
        },
      };
    }
    case "audio":
    case "obs":
    case "twitch":
    case "youtube":
    case "sequence":
      return { error: t("form.type_unsupported") };
  }
}

interface FormProps {
  draft: Draft;
  isNew: boolean;
  onChange: (draft: Draft) => void;
}

/** The name of a program for the form: the file without folder and `.exe`. */
const programName = (path: string): string => (path.split(/[\\/]/).pop() ?? path).replace(/\.exe$/i, "");

const EMPTY_PAGE = { id: "p", name: "", items: [] };
const NO_ACTIONS = { version: 1 as const, actions: {} };

/**
 * The form of one action (DESIGN.md 4.6a): the ID is never shown (it is made from the name), the type is picked from
 * icons, a program is chosen from a list, a shortcut is recorded, media and system actions are big tiles; arguments,
 * working folder and the toggle variable are under `More options`.
 */
export default function ActionForm({ draft, onChange }: FormProps) {
  const [picking, setPicking] = useState(false);
  const set = <K extends keyof Draft>(key: K, value: Draft[K]) => onChange({ ...draft, [key]: value });
  const keys = parseKeys(draft.keysText) ?? [];
  const mediaTiles = presetActions().filter((p) => p.action.type === "media");
  const systemTiles = presetActions().filter((p) => p.action.type === "system");
  // A name that was filled in by a tile or a program follows the choice; one the user typed is kept.
  const autoNames = new Set(["", ...presetActions().map((p) => p.label), programName(draft.program)]);
  const nameFor = (label: string) => (autoNames.has(draft.name.trim()) ? label : draft.name);

  return (
    <div>
      <Field label={t("form.name")}>
        <input value={draft.name} onChange={(e) => set("name", e.target.value)} placeholder={t("form.name_placeholder")} />
      </Field>

      <div className="type-seg" role="group" aria-label={t("form.type")}>
        {actionTypes().map((kind) => (
          <button key={kind.id} type="button" className={draft.type === kind.id ? "on" : ""} onClick={() => set("type", kind.id)}>
            <PhIcon name={kind.icon} size={18} />
            {kind.label}
          </button>
        ))}
      </div>

      {draft.type === "launch" && (
        <>
          <div className="program-row">
            <strong>{draft.program ? programName(draft.program) : t("form.no_program")}</strong>
            <button type="button" onClick={() => setPicking(true)}>
              {t("form.choose")}
            </button>
          </div>
          {draft.program && <small className="muted">{draft.program}</small>}
          {picking && (
            <QuickPicker
              anchor={{ left: window.innerWidth / 2 - 180, top: 120, bottom: 120 }}
              cell={{ x: 0, y: 0 }}
              page={EMPTY_PAGE}
              grid={{ cols: 3, rows: 2 }}
              actions={NO_ACTIONS}
              appsOnly
              onClose={() => setPicking(false)}
              onChoose={(choice) => {
                setPicking(false);
                if (choice.type === "app") onChange({ ...applyInstalledApp(draft, choice.app), name: nameFor(choice.app.name) });
              }}
            />
          )}
          <p className="warn-text">{t("form.launch_warning")}</p>
        </>
      )}

      {draft.type === "open_url" && (
        <Field label={t("form.address")} hint={t("form.address_hint")}>
          <input value={draft.url} onChange={(e) => set("url", e.target.value)} placeholder="github.com" />
        </Field>
      )}

      {draft.type === "hotkey" && (
        <Field label={t("form.keys")}>
          <ChordField keys={keys} onChange={(next) => set("keysText", formatKeys(next))} />
        </Field>
      )}

      {draft.type === "media" && (
        <div className="tile-grid" role="group" aria-label={t("form.type_media")}>
          {mediaTiles.map((p) => (
            <button
              key={p.id}
              type="button"
              className={p.action.type === "media" && p.action.op === draft.mediaOp ? "on" : ""}
              onClick={() => p.action.type === "media" && onChange({ ...draft, mediaOp: p.action.op, name: nameFor(p.label) })}
            >
              <PhIcon name={MEDIA_TILE_ICONS[p.id] ?? "play"} size={24} />
              {p.label}
            </button>
          ))}
        </div>
      )}

      {draft.type === "system" && (
        <div className="tile-grid" role="group" aria-label={t("form.type_system")}>
          {systemTiles.map((p) => (
            <button
              key={p.id}
              type="button"
              className={p.action.type === "system" && p.action.op === draft.systemOp ? "on" : ""}
              onClick={() => p.action.type === "system" && onChange({ ...draft, systemOp: p.action.op, name: nameFor(p.label) })}
            >
              <PhIcon name={p.id === "sleep" ? "moon" : "lock"} size={24} />
              {p.label}
            </button>
          ))}
        </div>
      )}

      {draft.type === "ha" && <HaFields entity={draft.haEntity} service={draft.haService} onChange={(haEntity, haService) => onChange({ ...draft, haEntity, haService })} />}

      {draft.type === "screenshot" && (
        <div className="tile-grid" role="group" aria-label={t("m29.type_screenshot")}>
          {SHOT_AREAS.map((area) => (
            <button
              key={area}
              type="button"
              className={draft.shotArea === area ? "on" : ""}
              onClick={() => onChange({ ...draft, shotArea: area, name: draft.name.trim() === "" || SHOT_AREAS.some((a) => t(`m29.shot_${a}`) === draft.name) ? t(`m29.shot_${area}`) : draft.name })}
            >
              <PhIcon name={SHOT_ICONS[area]} size={24} />
              {t(`m29.shot_${area}`)}
            </button>
          ))}
        </div>
      )}

      {draft.type === "http" && (
        <>
          <div className="tile-grid" role="group" aria-label={t("m29.webhook_method")}>
            {(["GET", "POST"] as const).map((method) => (
              <button key={method} type="button" className={draft.method === method ? "on" : ""} onClick={() => set("method", method)}>
                {method}
              </button>
            ))}
          </div>
          <Field label={t("form.address")} hint={t("m29.webhook_url_hint")}>
            <input value={draft.url} onChange={(e) => set("url", e.target.value)} placeholder="https://example.com/hook" />
          </Field>
          {draft.method === "POST" && (
            <Field label={t("m29.webhook_body")}>
              <textarea rows={3} value={draft.body} maxLength={4096} onChange={(e) => set("body", e.target.value)} />
            </Field>
          )}
          <Field label={t("m29.webhook_headers")} hint={t("m29.webhook_headers_hint")}>
            <textarea rows={2} value={draft.headersText} onChange={(e) => set("headersText", e.target.value)} placeholder="Content-Type: application/json" />
          </Field>
        </>
      )}

      <Disclosure title={t("common.more_options")}>
        {draft.type === "launch" && (
          <>
            <Field label={t("form.arguments")} hint={t("form.arguments_hint")}>
              <textarea rows={3} value={draft.argsText} onChange={(e) => set("argsText", e.target.value)} />
            </Field>
            <Field label={t("form.cwd")}>
              <input value={draft.cwd} onChange={(e) => set("cwd", e.target.value)} />
            </Field>
            <label className="switch-row">
              <Switch checked={draft.hidden} onChange={(checked) => set("hidden", checked)} label={t("form.hidden")} />
              <span>{t("form.hidden")}</span>
            </label>
          </>
        )}
        <Field label={t("form.toggle")} hint={t("form.toggle_hint")}>
          <input value={draft.toggleVar} onChange={(e) => set("toggleVar", e.target.value)} placeholder="audio.muted" />
        </Field>
      </Disclosure>
    </div>
  );
}

const MEDIA_TILE_ICONS: Record<string, string> = {
  "play-pause": "play-pause",
  "next-track": "skip-forward",
  "prev-track": "skip-back",
  "volume-up": "speaker-high",
  "volume-down": "speaker-low",
  mute: "speaker-slash",
};