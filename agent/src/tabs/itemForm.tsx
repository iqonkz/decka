import type { ActionsFile, ButtonItem, ButtonLook, LayoutItem, Page } from "@decka/shared";
import { call, navigate } from "../ipc";
import IconField from "./IconField";
import Button from "../ui/Button";
import Field from "../ui/Field";
import { normalPages, popupPages, sizeRange, TAP_MODE_LABELS, tapModeOf, tapModePatch, type Grid, type TapMode } from "./layoutOps";


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

/** An action of the `media` widget's key; `None` = no key. */
function ActionSelect({ label, value, actions, onChange }: { label: string; value: string | undefined; actions: ActionsFile; onChange: (id: string | undefined) => void }) {
  const entries = Object.entries(actions.actions).sort(([, a], [, b]) => a.name.localeCompare(b.name));
  return (
    <Field label={label}>
      <select value={value ?? ""} onChange={(e) => onChange(e.target.value || undefined)}>
        <option value="">None</option>
        {value !== undefined && !actions.actions[value] && <option value={value}>{value} (missing)</option>}
        {entries.map(([id, a]) => (
          <option key={id} value={id}>{`${a.name} (${id})`}</option>
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
      <IconField value={look.icon} onChange={(icon) => set("icon", icon ?? "")} />
      <input type="color" value={look.color ?? "#1e1e24"} onChange={(e) => set("color", e.target.value)} aria-label="Color" />
      <Button onClick={() => set("color", "")}>Clear</Button>
    </div>
  );
}

interface Props {
  item: LayoutItem;
  page: Page;
  pages: Page[];
  grid: Grid;
  actions: ActionsFile;
  onChange: (patch: Partial<LayoutItem>) => void;
  onDelete: () => void;
}

export default function ItemForm({ item, page, pages, grid, actions, onChange, onDelete }: Props) {
  const [minW, maxW, minH, maxH] = sizeRange(item.kind, grid);
  const patch = (p: Record<string, unknown>) => onChange(p as Partial<LayoutItem>);

  return (
    <div className="item-form">
      <h3>
        {item.kind} <code>{item.id}</code>
      </h3>
      <details className="advanced">
        <summary>Advanced: position and size</summary>
        <div className="grid-4">
          <NumberSelect label="X" value={item.x} options={range(0, grid.cols - item.w)} onChange={(x) => patch({ x })} />
          <NumberSelect label="Y" value={item.y} options={range(0, grid.rows - item.h)} onChange={(y) => patch({ y })} />
          <NumberSelect label="W" value={item.w} options={range(minW, maxW)} onChange={(w) => patch({ w })} />
          <NumberSelect label="H" value={item.h} options={range(minH, maxH)} onChange={(h) => patch({ h })} />
        </div>
      </details>

      {item.kind === "button" && <ButtonFields item={item} pages={pages} inPopup={page.popup === true} actions={actions} patch={patch} />}

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
      {item.kind === "media" && (
        <>
          <ActionSelect label="Previous" value={item.prevAction} actions={actions} onChange={(prevAction) => patch({ prevAction })} />
          <ActionSelect label="Play / Pause" value={item.playAction} actions={actions} onChange={(playAction) => patch({ playAction })} />
          <ActionSelect label="Next" value={item.nextAction} actions={actions} onChange={(nextAction) => patch({ nextAction })} />
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

function ButtonFields({ item, pages, inPopup, actions, patch }: { item: ButtonItem; pages: Page[]; inPopup: boolean; actions: ActionsFile; patch: (p: Record<string, unknown>) => void }) {
  const mode = tapModeOf(item);
  const normal = normalPages(pages);
  const popups = popupPages(pages);
  const actionEntries = Object.entries(actions.actions).sort(([, a], [, b]) => a.name.localeCompare(b.name));
  const states = Object.entries(item.bind?.states ?? {});
  const selectedAction = item.actionId !== undefined ? actions.actions[item.actionId] : undefined;
  const launchProgram = selectedAction?.type === "launch" ? selectedAction.program : null;

  /** Choosing a `launch` action on a button without an icon takes the program's icon (kept until Save). */
  function selectAction(id: string) {
    patch({ actionId: id });
    const chosen = actions.actions[id];
    if (chosen?.type === "launch" && !item.icon) {
      call("icon_from_program", { path: chosen.program }).then((r) => "icon" in r && patch({ icon: r.icon }), () => {});
    }
  }

  const setStates = (entries: [string, ButtonLook][]) => patch({ bind: { var: item.bind?.var ?? "", states: Object.fromEntries(entries) } });

  return (
    <>
      <Field label="Label">
        <input value={item.label ?? ""} maxLength={24} onChange={(e) => patch({ label: e.target.value || undefined })} />
      </Field>
      <Field label="Icon" hint="A Lucide icon, an emoji, the icon of the action's program, or a picture file.">
        <IconField value={item.icon} onChange={(icon) => patch({ icon })} program={launchProgram} />
      </Field>
      <Field label="Color">
        <div className="row">
          <input type="color" value={item.color ?? "#1e1e24"} onChange={(e) => patch({ color: e.target.value })} />
          <Button onClick={() => patch({ color: undefined })}>Clear</Button>
        </div>
      </Field>

      <Field label="On tap" hint={inPopup ? "Keys inside a popup group can only run an action." : undefined}>
        <select
          value={mode}
          onChange={(e) =>
            patch(tapModePatch(item, e.target.value as TapMode, { firstActionId: Object.keys(actions.actions)[0], firstPageId: normal[0]?.id, firstPopupId: popups[0]?.id }))
          }
        >
          <option value="action">{TAP_MODE_LABELS.action}</option>
          {!inPopup && <option value="page">{TAP_MODE_LABELS.page}</option>}
          {!inPopup && (
            <option value="popup" disabled={popups.length === 0}>
              {TAP_MODE_LABELS.popup}
              {popups.length === 0 ? " (add a popup group first)" : ""}
            </option>
          )}
          {!inPopup && (
            <option value="action-popup" disabled={popups.length === 0}>
              {TAP_MODE_LABELS["action-popup"]}
              {popups.length === 0 ? " (add a popup group first)" : ""}
            </option>
          )}
        </select>
      </Field>
      {(mode === "action" || mode === "action-popup") && (
        <>
          <Field label="Action" hint="New action… leaves this form: save the layout first.">
            <div className="row">
              <select value={item.actionId ?? ""} onChange={(e) => selectAction(e.target.value)}>
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
      )}
      {mode === "page" && (
        <Field label="Page">
          <select value={item.navigate ?? ""} onChange={(e) => patch({ navigate: e.target.value })}>
            {normal.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
              </option>
            ))}
          </select>
        </Field>
      )}
      {(mode === "popup" || mode === "action-popup") && (
        <Field label="Popup group">
          <select value={item.popup ?? ""} onChange={(e) => patch({ popup: e.target.value })}>
            {popups.map((p) => (
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
