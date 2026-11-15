import { t, type Action, type MediaOp } from "@decka/shared";
import { useState } from "react";
import Field from "../ui/Field";
import { presetActions } from "../editor/addButton";
import ChordField from "../editor/ChordField";
import QuickPicker from "../editor/QuickPicker";
import PhIcon from "../ui/PhIcon";
import { formatKeys, parseKeys } from "./keys";
import { applyInstalledApp, applyPickedProgram } from "./programPick";

export type ActionType = Action["type"];

export const MEDIA_OPS: MediaOp[] = ["play_pause", "next", "prev", "vol_up", "vol_down", "mute"];
/** The five kinds of action with their labels in the current language. */
export const actionTypes = (): { id: ActionType; label: string; icon: string }[] => [
  { id: "launch", label: t("form.type_app"), icon: "rocket-launch" },
  { id: "open_url", label: t("form.type_website"), icon: "globe" },
  { id: "hotkey", label: t("form.type_shortcut"), icon: "keyboard" },
  { id: "media", label: t("form.type_media"), icon: "play-circle" },
  { id: "system", label: t("form.type_system"), icon: "power" },
];

/** Everything the form edits, as strings and booleans. */
export interface Draft {
  id: string;
  name: string;
  type: ActionType;
  keysText: string;
  mediaOp: MediaOp;
  systemOp: "lock" | "sleep";
  program: string;
  argsText: string;
  cwd: string;
  hidden: boolean;
  url: string;
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

      <details className="advanced">
        <summary>{t("common.more_options")}</summary>
        {draft.type === "launch" && (
          <>
            <Field label={t("form.arguments")} hint={t("form.arguments_hint")}>
              <textarea rows={3} value={draft.argsText} onChange={(e) => set("argsText", e.target.value)} />
            </Field>
            <Field label={t("form.cwd")}>
              <input value={draft.cwd} onChange={(e) => set("cwd", e.target.value)} />
            </Field>
            <label className="check">
              <input type="checkbox" checked={draft.hidden} onChange={(e) => set("hidden", e.target.checked)} /> {t("form.hidden")}
            </label>
          </>
        )}
        <Field label={t("form.toggle")} hint={t("form.toggle_hint")}>
          <input value={draft.toggleVar} onChange={(e) => set("toggleVar", e.target.value)} placeholder="audio.muted" />
        </Field>
      </details>
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