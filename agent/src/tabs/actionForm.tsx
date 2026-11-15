import type { Action, MediaOp } from "@decka/shared";
import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import Field from "../ui/Field";
import AppPickerDialog from "./AppPickerDialog";
import { formatKeys, parseKeys } from "./keys";
import { applyInstalledApp, applyPickedProgram } from "./programPick";

export type ActionType = Action["type"];

export const MEDIA_OPS: MediaOp[] = ["play_pause", "next", "prev", "vol_up", "vol_down", "mute"];
export const ACTION_TYPES: { id: ActionType; label: string }[] = [
  { id: "hotkey", label: "Hotkey" },
  { id: "media", label: "Media key" },
  { id: "launch", label: "Launch a program" },
  { id: "open_url", label: "Open a URL" },
  { id: "system", label: "System (lock / sleep)" },
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
      if (!keys) return { error: "Keys: use 1 to 5 key names joined by +, for example ctrl+shift+m." };
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

export default function ActionForm({ draft, isNew, onChange }: FormProps) {
  const set = <K extends keyof Draft>(key: K, value: Draft[K]) => onChange({ ...draft, [key]: value });
  const [picking, setPicking] = useState(false);
  return (
    <div>
      {isNew && (
        <Field label="ID" hint="Lower-case letters, digits, _ and -. Cannot be changed later.">
          <input value={draft.id} onChange={(e) => set("id", e.target.value)} placeholder="open-notes" />
        </Field>
      )}
      <Field label="Name">
        <input value={draft.name} onChange={(e) => set("name", e.target.value)} />
      </Field>
      <Field label="Type">
        <select value={draft.type} onChange={(e) => set("type", e.target.value as ActionType)}>
          {ACTION_TYPES.map((t) => (
            <option key={t.id} value={t.id}>
              {t.label}
            </option>
          ))}
        </select>
      </Field>

      {draft.type === "hotkey" && (
        <Field label="Keys" hint="Joined by +, for example ctrl+shift+m. Up to 5 keys.">
          <input value={draft.keysText} onChange={(e) => set("keysText", e.target.value)} placeholder="ctrl+shift+m" />
        </Field>
      )}
      {draft.type === "media" && (
        <Field label="Operation">
          <select value={draft.mediaOp} onChange={(e) => set("mediaOp", e.target.value as MediaOp)}>
            {MEDIA_OPS.map((op) => (
              <option key={op} value={op}>
                {op}
              </option>
            ))}
          </select>
        </Field>
      )}
      {draft.type === "launch" && (
        <>
          <Field label="Program" hint="Shortcuts (.lnk) are not supported: pick the .exe itself.">
            <div className="row">
              <input value={draft.program} onChange={(e) => set("program", e.target.value)} placeholder="notepad.exe" />
              <button
                type="button"
                onClick={() =>
                  void open({ multiple: false, directory: false, filters: [{ name: "Programs", extensions: ["exe"] }] }).then((picked) => {
                    if (typeof picked === "string") onChange(applyPickedProgram(draft, picked));
                  })
                }
              >
                Browse…
              </button>
              <button type="button" onClick={() => setPicking(true)}>
                Choose installed app…
              </button>
            </div>
          </Field>
          {picking && (
            <AppPickerDialog
              onClose={() => setPicking(false)}
              onPick={(app) => {
                setPicking(false);
                onChange(applyInstalledApp(draft, app));
              }}
            />
          )}
          <Field label="Arguments" hint="One argument per line.">
            <textarea rows={3} value={draft.argsText} onChange={(e) => set("argsText", e.target.value)} />
          </Field>
          <Field label="Working directory">
            <input value={draft.cwd} onChange={(e) => set("cwd", e.target.value)} />
          </Field>
          <label className="check">
            <input type="checkbox" checked={draft.hidden} onChange={(e) => set("hidden", e.target.checked)} /> Hide window
          </label>
        </>
      )}
      {draft.type === "open_url" && (
        <Field label="URL" hint="http or https only.">
          <input value={draft.url} onChange={(e) => set("url", e.target.value)} placeholder="https://example.com" />
        </Field>
      )}
      {draft.type === "system" && (
        <Field label="Operation">
          <select value={draft.systemOp} onChange={(e) => set("systemOp", e.target.value as "lock" | "sleep")}>
            <option value="lock">lock</option>
            <option value="sleep">sleep</option>
          </select>
        </Field>
      )}

      <Field label="Toggle variable" hint="Optional. After a successful run this on/off variable flips; buttons can change their look with it.">
        <input value={draft.toggleVar} onChange={(e) => set("toggleVar", e.target.value)} placeholder="audio.muted" />
      </Field>

      {draft.type === "launch" && <p className="warn-text">This program will run on your PC whenever the button is pressed on a paired phone.</p>}
    </div>
  );
}
