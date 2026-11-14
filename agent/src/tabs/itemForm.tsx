import type { ActionsFile, ButtonItem, ButtonLook, LayoutItem, Page } from "@decka/shared";
import { navigate } from "../ipc";
import Button from "../ui/Button";
import Field from "../ui/Field";
import { COLS, ROWS, SIZE_RANGE } from "./layoutOps";

/** The 60 Lucide icons the client ships (DATA_MODEL.md 3.6). */
export const ICON_NAMES = [
  "play", "pause", "skip-back", "skip-forward", "volume", "volume-1", "volume-2", "volume-x", "mic", "mic-off",
  "video", "video-off", "monitor", "lock", "power", "moon", "sun", "home", "settings", "terminal",
  "code", "link", "git-branch", "globe", "folder", "file-text", "activity", "cpu", "hard-drive", "wifi",
  "bell", "bell-off", "mail", "message-square", "music", "headphones", "camera", "image", "calendar", "clock",
  "timer", "star", "heart", "zap", "rocket", "bug", "check", "x", "plus", "minus",
  "refresh-cw", "search", "download", "upload", "copy", "trash-2", "square", "circle", "arrow-left", "arrow-right",
];

const range = (from: number, to: number): number[] => Array.from({ length: Math.max(0, to - from + 1) }, (_, i) => from + i);

function NumberSelect({ label, value, options, onChange }: { label: string; value: number; options: number[]; onChange: (n: number) => void }) {
  const list = options.includes(value) ? options : [...options, value].sort((a, b) => a - b);
  return (
    <Field label={label}>
      <select value={value} disabled={list.length === 1} onChange={(e) => onChange(Number(e.target.value))}>
        {list.map((n) => (
          <option key={n} value={n}>
            {n}
          </option>
        ))}
      </select>
    </Field>
  );
}

function LookFields({ look, onChange }: { look: ButtonLook; onChange: (look: ButtonLook) => void }) {
  const set = (key: keyof ButtonLook, value: string) => {
    const next = { ...look };
    if (value === "") delete next[key];
    else next[key] = value;
    onChange(next);
  };
  return (
    <div className="look-fields">
      <input placeholder="Label" value={look.label ?? ""} maxLength={24} onChange={(e) => set("label", e.target.value)} />
      <input placeholder="lucide:play" value={look.icon ?? ""} onChange={(e) => set("icon", e.target.value)} />
      <input type="color" value={look.color ?? "#1e1e24"} onChange={(e) => set("color", e.target.value)} aria-label="Color" />
      <Button onClick={() => set("color", "")}>Clear</Button>
    </div>
  );
}

interface Props {
  item: LayoutItem;
  page: Page;
  pages: Page[];
  actions: ActionsFile;
  onChange: (patch: Partial<LayoutItem>) => void;
  onDelete: () => void;
}

export default function ItemForm({ item, pages, actions, onChange, onDelete }: Props) {
  const [minW, maxW, minH, maxH] = SIZE_RANGE[item.kind];
  const patch = (p: Record<string, unknown>) => onChange(p as Partial<LayoutItem>);

  return (
    <div className="item-form">
      <h3>
        {item.kind} <code>{item.id}</code>
      </h3>
      <div className="grid-4">
        <NumberSelect label="X" value={item.x} options={range(0, COLS - item.w)} onChange={(x) => patch({ x })} />
        <NumberSelect label="Y" value={item.y} options={range(0, ROWS - item.h)} onChange={(y) => patch({ y })} />
        <NumberSelect label="W" value={item.w} options={range(minW, maxW)} onChange={(w) => patch({ w })} />
        <NumberSelect label="H" value={item.h} options={range(minH, maxH)} onChange={(h) => patch({ h })} />
      </div>

      {item.kind === "button" && <ButtonFields item={item} pages={pages} actions={actions} patch={patch} />}

      {item.kind === "clock" && (
        <label className="check">
          <input type="checkbox" checked={item.seconds === true} onChange={(e) => patch({ seconds: e.target.checked || undefined })} /> Show seconds
        </label>
      )}
      {item.kind === "statuses" && (
        <>
          <Field label="Title">
            <input value={item.title ?? ""} maxLength={30} onChange={(e) => patch({ title: e.target.value || undefined })} />
          </Field>
          <Field label="Group" hint="Show only statuses of this group. Empty = all.">
            <input value={item.group ?? ""} onChange={(e) => patch({ group: e.target.value || undefined })} />
          </Field>
        </>
      )}
      {item.kind === "status" && (
        <Field label="Status ID">
          <input value={item.statusId} onChange={(e) => patch({ statusId: e.target.value })} />
        </Field>
      )}

      <Button variant="danger" onClick={onDelete}>
        Delete item
      </Button>
    </div>
  );
}

function ButtonFields({ item, pages, actions, patch }: { item: ButtonItem; pages: Page[]; actions: ActionsFile; patch: (p: Record<string, unknown>) => void }) {
  const mode: "action" | "page" = item.navigate !== undefined ? "page" : "action";
  const actionEntries = Object.entries(actions.actions).sort(([, a], [, b]) => a.name.localeCompare(b.name));
  const states = Object.entries(item.bind?.states ?? {});

  const setStates = (entries: [string, ButtonLook][]) => patch({ bind: { var: item.bind?.var ?? "", states: Object.fromEntries(entries) } });

  return (
    <>
      <Field label="Label">
        <input value={item.label ?? ""} maxLength={24} onChange={(e) => patch({ label: e.target.value || undefined })} />
      </Field>
      <Field label="Icon" hint="lucide:<name> or emoji:<symbol>">
        <div className="row">
          <input value={item.icon ?? ""} onChange={(e) => patch({ icon: e.target.value || undefined })} />
          <select value="" aria-label="Pick an icon" onChange={(e) => e.target.value && patch({ icon: `lucide:${e.target.value}` })}>
            <option value="">Pick…</option>
            {ICON_NAMES.map((n) => (
              <option key={n} value={n}>
                {n}
              </option>
            ))}
          </select>
        </div>
      </Field>
      <Field label="Color">
        <div className="row">
          <input type="color" value={item.color ?? "#1e1e24"} onChange={(e) => patch({ color: e.target.value })} />
          <Button onClick={() => patch({ color: undefined })}>Clear</Button>
        </div>
      </Field>

      <div className="row radios">
        <label>
          <input
            type="radio"
            checked={mode === "action"}
            onChange={() => patch({ navigate: undefined, confirm: undefined, actionId: item.actionId ?? Object.keys(actions.actions)[0] })}
          />{" "}
          Runs action
        </label>
        <label>
          <input type="radio" checked={mode === "page"} onChange={() => patch({ actionId: undefined, confirm: undefined, navigate: pages[0]?.id })} /> Opens page
        </label>
      </div>
      {mode === "action" ? (
        <>
          <Field label="Action" hint="New action… leaves this form: save the layout first.">
            <div className="row">
              <select value={item.actionId ?? ""} onChange={(e) => patch({ actionId: e.target.value })}>
                {item.actionId !== undefined && !actions.actions[item.actionId] && <option value={item.actionId}>{item.actionId} (missing)</option>}
                {actionEntries.map(([id, a]) => (
                  <option key={id} value={id}>{`${a.name} (${id})`}</option>
                ))}
              </select>
              <button type="button" onClick={() => void navigate("actions:new")}>
                New action…
              </button>
            </div>
          </Field>
          <label className="check">
            <input type="checkbox" checked={item.confirm === true} onChange={(e) => patch({ confirm: e.target.checked || undefined })} /> Hold to confirm
          </label>
        </>
      ) : (
        <Field label="Page">
          <select value={item.navigate ?? ""} onChange={(e) => patch({ navigate: e.target.value })}>
            {pages.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
              </option>
            ))}
          </select>
        </Field>
      )}

      <div className="bind-block">
        <h3>State binding</h3>
        <Field label="Variable" hint="The look changes with this variable's value, for example audio.muted.">
          <input
            value={item.bind?.var ?? ""}
            onChange={(e) => (e.target.value === "" && states.length === 0 ? patch({ bind: undefined }) : patch({ bind: { var: e.target.value, states: Object.fromEntries(states) } }))}
          />
        </Field>
        {states.map(([key, look], i) => (
          <div className="state-row" key={i}>
            <input
              className="state-key"
              aria-label="Value"
              placeholder="true"
              value={key}
              onChange={(e) => setStates(states.map(([k, l], n): [string, ButtonLook] => (n === i ? [e.target.value, l] : [k, l])))}
            />
            <LookFields look={look} onChange={(next) => setStates(states.map(([k, l], n): [string, ButtonLook] => (n === i ? [k, next] : [k, l])))} />
            <Button onClick={() => setStates(states.filter((_, n) => n !== i))}>Remove</Button>
          </div>
        ))}
        <Button
          disabled={states.length >= 8}
          onClick={() => setStates([...states, [states.some(([k]) => k === "true") ? "false" : "true", {}]])}
        >
          Add state
        </Button>
      </div>
    </>
  );
}
