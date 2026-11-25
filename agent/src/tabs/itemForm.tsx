import { useEffect, useState } from "react";
import { t, type ActionsFile, type ButtonItem, type ButtonLook, type LayoutItem, type Page } from "@decka/shared";
import { call, navigate } from "../ipc";
import IconField from "./IconField";
import Button from "../ui/Button";
import Disclosure from "../ui/Disclosure";
import Field from "../ui/Field";
import Select from "../ui/Select";
import Switch from "../ui/Switch";
import type { DisplaySettings } from "@decka/shared";
import KeyPreview from "./KeyPreview";
import { describeAction } from "../editor/describeAction";
import { normalPages, popupPages, sizeRange, tapModeLabels, tapModeOf, tapModePatch, type Grid, type TapMode } from "./layoutOps";


const range = (from: number, to: number): number[] => Array.from({ length: Math.max(0, to - from + 1) }, (_, i) => from + i);

function NumberSelect({ label, value, options, onChange }: { label: string; value: number; options: number[]; onChange: (n: number) => void }) {
  const list = options.includes(value) ? options : [...options, value].sort((a, b) => a - b);
  return (
    <Field label={label}>
      <Select
        aria-label={label}
        value={String(value)}
        disabled={list.length === 1}
        options={list.map((n) => ({ value: String(n), label: String(n) }))}
        onChange={(val) => onChange(Number(val))}
      />
    </Field>
  );
}

/** The `list` widget's choice (DATA_MODEL.md 13.7): a drop-down of the names of the lists the PC has. */
function ListPicker({ value, onChange }: { value: string | undefined; onChange: (id: string | undefined) => void }) {
  const [lists, setLists] = useState<{ id: string; name: string }[]>([]);
  useEffect(() => {
    let alive = true;
    call("lists_get").then((l) => alive && setLists(l.map(({ id, name }) => ({ id, name })))).catch(() => undefined);
    return () => {
      alive = false;
    };
  }, []);
  const opts = [
    { value: "", label: t("item.list_first") },
    ...(value !== undefined && !lists.some((l) => l.id === value) ? [{ value, label: t("item.missing", { id: value }) }] : []),
    ...lists.map((l) => ({ value: l.id, label: l.name })),
  ];
  return (
    <Field label={t("item.list")}>
      <Select aria-label={t("item.list")} value={value ?? ""} options={opts} onChange={(val) => onChange(val || undefined)} />
    </Field>
  );
}

/** An action of the `media` widget's key; `None` = no key. */
function ActionSelect({ label, value, actions, onChange }: { label: string; value: string | undefined; actions: ActionsFile; onChange: (id: string | undefined) => void }) {
  const entries = Object.entries(actions.actions).sort(([, a], [, b]) => a.name.localeCompare(b.name));
  const opts = [
    { value: "", label: t("item.none") },
    ...(value !== undefined && !actions.actions[value] ? [{ value, label: t("item.missing", { id: value }) }] : []),
    ...entries.map(([id, a]) => ({ value: id, label: `${a.name} (${id})` })),
  ];
  return (
    <Field label={label}>
      <Select aria-label={label} value={value ?? ""} options={opts} onChange={(val) => onChange(val || undefined)} />
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
      <input placeholder={t("item.label")} value={look.label ?? ""} maxLength={24} onChange={(e) => set("label", e.target.value)} />
      <IconField value={look.icon} onChange={(icon) => set("icon", icon ?? "")} />
      <input type="color" value={look.color ?? "#1e1e24"} onChange={(e) => set("color", e.target.value)} aria-label={t("menu_item.color")} />
      <Button onClick={() => set("color", "")}>{t("item.clear")}</Button>
    </div>
  );
}

interface Props {
  item: LayoutItem;
  display: DisplaySettings;
  page: Page;
  pages: Page[];
  grid: Grid;
  actions: ActionsFile;
  onChange: (patch: Partial<LayoutItem>) => void;
  onDelete: () => void;
  /** Opens the quick picker to give this button another action. */
  onChangeAction: () => void;
}

const SWATCHES = ["#7c5cff", "#3b82f6", "#06b6d4", "#16a34a", "#eab308", "#f97316", "#ef4444", "#ec4899"];

export default function ItemForm({ item, display, page, pages, grid, actions, onChange, onDelete, onChangeAction }: Props) {
  const [minW, maxW, minH, maxH] = sizeRange(item.kind, grid);
  const patch = (p: Record<string, unknown>) => onChange(p as Partial<LayoutItem>);
  const position = (
    <div className="grid-4">
      <NumberSelect label="X" value={item.x} options={range(0, grid.cols - item.w)} onChange={(x) => patch({ x })} />
      <NumberSelect label="Y" value={item.y} options={range(0, grid.rows - item.h)} onChange={(y) => patch({ y })} />
      <NumberSelect label="W" value={item.w} options={range(minW, maxW)} onChange={(w) => patch({ w })} />
      <NumberSelect label="H" value={item.h} options={range(minH, maxH)} onChange={(h) => patch({ h })} />
    </div>
  );

  return (
    <div className="item-form-host">
    <div className="item-form">
      <h3>
        {item.kind === "button" ? t("item.button") : t("item.widget")}
      </h3>
      {item.kind === "button" && <KeyPreview item={item} display={display} />}
      {item.kind === "button" && <ButtonFields display={display} item={item} pages={pages} inPopup={page.popup === true} actions={actions} patch={patch} onChangeAction={onChangeAction} position={position} />}
      {item.kind !== "button" && (
        <Disclosure title={t("item.position_size")}>
          {position}
        </Disclosure>
      )}

      {item.kind === "clock" && (
        <label className="switch-row">
          <Switch checked={item.seconds === true} label={t("item.seconds")} onChange={(checked) => patch({ seconds: checked || undefined })} />
          <span>{t("item.seconds")}</span>
        </label>
      )}
      {item.kind === "statuses" && (
        <>
          <Field label={t("item.title")}>
            <input value={item.title ?? ""} maxLength={30} onChange={(e) => patch({ title: e.target.value || undefined })} />
          </Field>
          <Field label={t("integ.group")} hint={t("item.group_hint")}>
            <input value={item.group ?? ""} onChange={(e) => patch({ group: e.target.value || undefined })} />
          </Field>
        </>
      )}
      {item.kind === "media" && (
        <>
          <ActionSelect label={t("item.prev")} value={item.prevAction} actions={actions} onChange={(prevAction) => patch({ prevAction })} />
          <ActionSelect label={t("preset.play_pause")} value={item.playAction} actions={actions} onChange={(playAction) => patch({ playAction })} />
          <ActionSelect label={t("item.next")} value={item.nextAction} actions={actions} onChange={(nextAction) => patch({ nextAction })} />
        </>
      )}
      {item.kind === "ai" && (
        <Field label={t("item.ai_show")}>
          <Select
            aria-label={t("item.ai_show")}
            value={item.show ?? "both"}
            options={[
              { value: "both", label: t("item.ai_both") },
              { value: "claude", label: t("ai.claude") },
              { value: "codex", label: t("ai.codex") },
            ]}
            onChange={(val) => patch({ show: val === "both" ? undefined : val })}
          />
        </Field>
      )}
      {item.kind === "list" && <ListPicker value={item.listId} onChange={(listId) => patch({ listId })} />}
      {item.kind === "status" && (
        <Field label={t("item.status_id")}>
          <input value={item.statusId} onChange={(e) => patch({ statusId: e.target.value })} />
        </Field>
      )}

      <Button variant="danger" onClick={onDelete}>
        {t("item.delete")}
      </Button>
    </div>
    </div>
  );
}

function ButtonFields({ display, item, pages, inPopup, actions, patch, onChangeAction, position }: { display: DisplaySettings; item: ButtonItem; pages: Page[]; inPopup: boolean; actions: ActionsFile; patch: (p: Record<string, unknown>) => void; onChangeAction: () => void; position: React.ReactNode }) {
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
      <div className="form-cols">
      <div className="form-icons">
        <Field label={t("item.icon")}>
          <IconField value={item.icon} onChange={(icon) => patch({ icon })} program={launchProgram} onBrand={(icon, hex) => patch(item.color === undefined ? { icon, color: hex } : { icon })} />
        </Field>
        <div className="seg" role="group" aria-label={t("look.key_style")}>
          {(["standard", "full"] as const).map((face) => (
            <button key={face} type="button" className={(item.face ?? display.keyFace ?? "standard") === face ? "on" : ""} onClick={() => patch({ face })}>
              {face === "standard" ? t("look.standard") : t("menu_item.big_icon")}
            </button>
          ))}
        </div>
      </div>
      <div className="form-main">
      <Field label={t("form.name")}>
        <input value={item.label ?? ""} maxLength={24} onChange={(e) => patch({ label: e.target.value || undefined })} />
      </Field>
      <div className="does-card">
        <span className="muted">{t("item.does")}</span>
        <strong title={describeAction(selectedAction)}>{describeAction(selectedAction)}</strong>
        <Button onClick={onChangeAction}>{t("item.change")}</Button>
      </div>
      <Field label={t("menu_item.color")}>
        <div className="swatches">
          {SWATCHES.map((c) => (
            <button key={c} type="button" aria-label={`${t("menu_item.color")} ${c}`} className={`swatch${item.color?.toLowerCase() === c ? " on" : ""}`} style={{ background: c }} onClick={() => patch({ color: c })} />
          ))}
          <input type="color" aria-label={t("item.custom_color")} value={item.color ?? "#1e1e24"} onChange={(e) => patch({ color: e.target.value })} />
          <Button onClick={() => patch({ color: undefined })}>{t("menu_item.default")}</Button>
        </div>
      </Field>
      <label className="switch-row">
        <Switch checked={item.confirm === true} label={t("menu_item.ask")} onChange={(checked) => patch({ confirm: checked || undefined })} />
        <span>{t("menu_item.ask")}</span>
      </label>
      </div>
      </div>

      <Disclosure title={t("common.more_options")}>
        {position}
      <Field label={t("item.on_tap")} hint={inPopup ? t("item.on_tap_popup_hint") : undefined}>
        <Select
          aria-label={t("item.on_tap")}
          value={mode}
          options={[
            { value: "action", label: tapModeLabels().action },
            ...(!inPopup
              ? [
                  { value: "page", label: tapModeLabels().page },
                  {
                    value: "popup",
                    disabled: popups.length === 0,
                    label: `${tapModeLabels().popup}${popups.length === 0 ? ` ${t("item.add_group_first")}` : ""}`,
                  },
                  {
                    value: "action-popup",
                    disabled: popups.length === 0,
                    label: `${tapModeLabels()["action-popup"]}${popups.length === 0 ? ` ${t("item.add_group_first")}` : ""}`,
                  },
                ]
              : []),
          ]}
          onChange={(val) =>
            patch(tapModePatch(item, val as TapMode, { firstActionId: Object.keys(actions.actions)[0], firstPageId: normal[0]?.id, firstPopupId: popups[0]?.id }))
          }
        />
      </Field>
      {(mode === "action" || mode === "action-popup") && (
        <>
          <Field label={t("actions.action")} hint={t("item.new_action_hint")}>
            <div className="row">
              <Select
                aria-label={t("actions.action")}
                value={item.actionId ?? ""}
                options={[
                  ...(item.actionId !== undefined && !actions.actions[item.actionId]
                    ? [{ value: item.actionId, label: t("item.deleted_action") }]
                    : []),
                  ...actionEntries.map(([id, a]) => ({ value: id, label: `${a.name} (${id})` })),
                ]}
                onChange={(val) => selectAction(val)}
              />
              <Button onClick={() => void navigate("actions:new")}>
                {t("item.new_action")}
              </Button>
            </div>
          </Field>
        </>
      )}
      {mode === "page" && (
        <Field label={t("item.page")}>
          <Select
            aria-label={t("item.page")}
            value={item.navigate ?? ""}
            options={normal.map((p) => ({ value: p.id, label: p.name }))}
            onChange={(val) => patch({ navigate: val })}
          />
        </Field>
      )}
      {(mode === "popup" || mode === "action-popup") && (
        <Field label={t("layout.group_title")}>
          <Select
            aria-label={t("layout.group_title")}
            value={item.popup ?? ""}
            options={popups.map((p) => ({ value: p.id, label: p.name }))}
            onChange={(val) => patch({ popup: val })}
          />
        </Field>
      )}

      <div className="bind-block">
        <h3>{t("item.binding")}</h3>
        <Field label={t("item.variable")} hint={t("item.variable_hint")}>
          <input
            value={item.bind?.var ?? ""}
            onChange={(e) => (e.target.value === "" && states.length === 0 ? patch({ bind: undefined }) : patch({ bind: { var: e.target.value, states: Object.fromEntries(states) } }))}
          />
        </Field>
        {states.map(([key, look], i) => (
          <div className="state-row" key={i}>
            <input
              className="state-key"
              aria-label={t("item.value")}
              placeholder="true"
              value={key}
              onChange={(e) => setStates(states.map(([k, l], n): [string, ButtonLook] => (n === i ? [e.target.value, l] : [k, l])))}
            />
            <LookFields look={look} onChange={(next) => setStates(states.map(([k, l], n): [string, ButtonLook] => (n === i ? [k, next] : [k, l])))} />
            <Button onClick={() => setStates(states.filter((_, n) => n !== i))}>{t("integ.remove")}</Button>
          </div>
        ))}
        <Button
          disabled={states.length >= 8}
          onClick={() => setStates([...states, [states.some(([k]) => k === "true") ? "false" : "true", {}]])}
        >
          {t("item.add_state")}
        </Button>
      </div>
      </Disclosure>
    </>
  );
}
