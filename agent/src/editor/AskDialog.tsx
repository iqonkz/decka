import { useEffect, useState } from "react";
import { getLanguage, pickText, t, type Answer, type CatalogItem, type HaStatus, type Page } from "@decka/shared";
import { call } from "../ipc";
import Button from "../ui/Button";
import Modal from "../ui/Modal";
import Select from "../ui/Select";
import { actionFromUrl } from "./addButton";
import ChordField from "./ChordField";

export interface AskProps {
  item: CatalogItem;
  /** The pages of the deck: `page` and `popup` items ask for one of them. */
  pages: Page[];
  /** The page the key will be on (it cannot open itself). */
  currentPageId: string;
  /** OBS scenes and inputs; `null` = OBS is not connected. */
  obs: { scenes: string[]; inputs: string[] } | null;
  onConfirm: (answer: Answer) => void;
  onClose: () => void;
  onOpenObs: () => void;
  onOpenHa?: () => void;
  /** Tests give the answer of `ha_get`. */
  initialHa?: HaStatus;
}

/** The little dialog for what a library item cannot know (DESIGN.md 4.8): an address, keys, a scene, an input, a page, a device. */
export default function AskDialog({ item, pages, currentPageId, obs, onConfirm, onClose, onOpenObs, onOpenHa, initialHa }: AskProps) {
  const lang = getLanguage();
  const [url, setUrl] = useState("");
  const [keys, setKeys] = useState<string[]>([]);
  const [text, setText] = useState("");
  const [choice, setChoice] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [ha, setHa] = useState<HaStatus | null>(initialHa ?? null);
  useEffect(() => {
    if (item.ask !== "ha_entity" || initialHa) return;
    let active = true;
    call("ha_get").then(
      (s) => active && setHa(s),
      (e) => active && setError(String(e)),
    );
    return () => {
      active = false;
    };
  }, [item.ask, initialHa]);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  // A scene button offers scenes, a device button everything else.
  const wantsScene = item.action?.type === "ha" && item.action.service === "scene";
  const devices = (ha?.entities ?? []).filter((e) => e.id.startsWith("scene.") === wantsScene);
  const options =
    item.ask === "ha_entity" ? devices.map((e) => ({ id: e.id, label: e.name === e.id ? e.id : `${e.name} (${e.id})` }))
    : item.ask === "obs_scene" ? (obs?.scenes ?? []).map((s) => ({ id: s, label: s }))
    : item.ask === "obs_input" ? (obs?.inputs ?? []).map((s) => ({ id: s, label: s }))
    : pages.filter((p) => (item.popup === true ? p.popup === true : p.popup !== true && p.id !== currentPageId)).map((p) => ({ id: p.id, label: p.name }));

  const title = item.ask === "ha_entity" ? t("ha.entity") : item.ask === "text" ? t("ask.text") : item.ask === "url" ? t("ask.website") : item.ask === "keys" ? t("picker.shortcut_title") : item.ask === "obs_scene" ? t("ask.scene") : item.ask === "obs_input" ? t("ask.input") : item.popup ? t("ask.group") : t("ask.page");

  function confirm() {
    if (item.ask === "url") {
      const action = actionFromUrl(url);
      return action && action.type === "open_url" ? onConfirm({ url: action.url }) : setError(t("picker.enter_url"));
    }
    if (item.ask === "keys") return keys.length > 0 ? onConfirm({ keys }) : setError(t("picker.press_keys"));
    if (item.ask === "text") return text.trim() !== "" ? onConfirm({ text: text.trim() }) : setError(t("ask.enter_text"));
    if (choice === "") return setError(t("ask.choose"));
    if (item.ask === "ha_entity") return onConfirm({ entity: choice, entityName: devices.find((e) => e.id === choice)?.name ?? choice });
    if (item.ask === "obs_scene") return onConfirm({ scene: choice });
    if (item.ask === "obs_input") return onConfirm({ input: choice });
    return onConfirm({ page: choice });
  }

  const needsObs = (item.ask === "obs_scene" || item.ask === "obs_input") && obs === null;
  const needsHa = item.ask === "ha_entity" && ha !== null && !ha.hasToken;
  const empty = !needsObs && !needsHa && (item.ask === "obs_scene" || item.ask === "obs_input" || item.ask === "page" || (item.ask === "ha_entity" && ha !== null)) && options.length === 0;
  return (
    <Modal
      title={pickText(item.name, lang)}
      subtitle={title}
      onClose={onClose}
      className="ask-dialog"
      footer={
        <>
          <Button onClick={onClose}>{t("common.cancel")}</Button>
          <Button variant="primary" disabled={needsObs || needsHa || empty} onClick={confirm}>{t("library.add")}</Button>
        </>
      }
    >
      {item.ask === "url" && (
        <input autoFocus placeholder="github.com" aria-label={title} value={url} onChange={(e) => { setUrl(e.target.value); setError(null); }} onKeyDown={(e) => e.key === "Enter" && confirm()} />
      )}
      {item.ask === "text" && (
        <input autoFocus maxLength={item.action?.type === "youtube" ? 200 : 500} aria-label={title} value={text} onChange={(e) => { setText(e.target.value); setError(null); }} onKeyDown={(e) => e.key === "Enter" && confirm()} />
      )}
      {item.ask === "keys" && <ChordField keys={keys} onChange={(k) => { setKeys(k); setError(null); }} />}
      {needsObs && (
        <>
          <p className="muted">{t("ask.connect_obs")}</p>
          <div className="row"><Button onClick={onOpenObs}>{t("ask.open_obs")}</Button></div>
        </>
      )}
      {needsHa && (
        <>
          <p className="muted">{t("ha.not_set")}</p>
          {onOpenHa && <div className="row"><Button onClick={onOpenHa}>{t("ha.open")}</Button></div>}
        </>
      )}
      {!needsObs && !needsHa && item.ask !== "url" && item.ask !== "keys" && item.ask !== "text" && (
        empty ? (
          <p className="muted">{item.ask === "page" ? t("ask.no_pages") : item.ask === "ha_entity" ? (ha?.error ?? t("ha.no_entities")) : t("ask.connect_obs")}</p>
        ) : (
          <Select
            autoFocus
            aria-label={title}
            value={choice}
            placeholder={t("ask.choose")}
            options={options.map((o) => ({ value: o.id, label: o.label }))}
            onChange={(val) => { setChoice(val); setError(null); }}
          />
        )
      )}
      {error && <small className="warn-text">{error}</small>}
    </Modal>
  );
}
