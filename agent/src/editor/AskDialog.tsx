import { useState } from "react";
import { getLanguage, t, type Answer, type CatalogItem, type Page } from "@decka/shared";
import Button from "../ui/Button";
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
}

/** The little dialog for what a library item cannot know (DESIGN.md 4.8): an address, keys, a scene, an input, a page. */
export default function AskDialog({ item, pages, currentPageId, obs, onConfirm, onClose, onOpenObs }: AskProps) {
  const lang = getLanguage();
  const [url, setUrl] = useState("");
  const [keys, setKeys] = useState<string[]>([]);
  const [text, setText] = useState("");
  const [choice, setChoice] = useState("");
  const [error, setError] = useState<string | null>(null);

  const options =
    item.ask === "obs_scene" ? (obs?.scenes ?? []).map((s) => ({ id: s, label: s }))
    : item.ask === "obs_input" ? (obs?.inputs ?? []).map((s) => ({ id: s, label: s }))
    : pages.filter((p) => (item.popup === true ? p.popup === true : p.popup !== true && p.id !== currentPageId)).map((p) => ({ id: p.id, label: p.name }));

  const title = item.ask === "text" ? t("ask.text") : item.ask === "url" ? t("ask.website") : item.ask === "keys" ? t("picker.shortcut_title") : item.ask === "obs_scene" ? t("ask.scene") : item.ask === "obs_input" ? t("ask.input") : item.popup ? t("ask.group") : t("ask.page");

  function confirm() {
    if (item.ask === "url") {
      const action = actionFromUrl(url);
      return action && action.type === "open_url" ? onConfirm({ url: action.url }) : setError(t("picker.enter_url"));
    }
    if (item.ask === "keys") return keys.length > 0 ? onConfirm({ keys }) : setError(t("picker.press_keys"));
    if (item.ask === "text") return text.trim() !== "" ? onConfirm({ text: text.trim() }) : setError(t("ask.enter_text"));
    if (choice === "") return setError(t("ask.choose"));
    if (item.ask === "obs_scene") return onConfirm({ scene: choice });
    if (item.ask === "obs_input") return onConfirm({ input: choice });
    return onConfirm({ page: choice });
  }

  const needsObs = (item.ask === "obs_scene" || item.ask === "obs_input") && obs === null;
  const empty = !needsObs && (item.ask === "obs_scene" || item.ask === "obs_input" || item.ask === "page") && options.length === 0;
  return (
    <div className="dialog-backdrop" onPointerDown={onClose}>
      <div className="dialog ask-dialog" role="dialog" aria-label={title} onPointerDown={(e) => e.stopPropagation()}>
        <h2>{item.name[lang]}</h2>
        <strong>{title}</strong>
        {item.ask === "url" && (
          <input autoFocus placeholder="github.com" aria-label={title} value={url} onChange={(e) => { setUrl(e.target.value); setError(null); }} onKeyDown={(e) => e.key === "Enter" && confirm()} />
        )}
        {item.ask === "text" && (
          <input autoFocus maxLength={500} aria-label={title} value={text} onChange={(e) => { setText(e.target.value); setError(null); }} onKeyDown={(e) => e.key === "Enter" && confirm()} />
        )}
        {item.ask === "keys" && <ChordField keys={keys} onChange={(k) => { setKeys(k); setError(null); }} />}
        {needsObs && (
          <>
            <p className="muted">{t("ask.connect_obs")}</p>
            <div className="row"><Button onClick={onOpenObs}>{t("ask.open_obs")}</Button></div>
          </>
        )}
        {!needsObs && item.ask !== "url" && item.ask !== "keys" && item.ask !== "text" && (
          empty ? (
            <p className="muted">{item.ask === "page" ? t("ask.no_pages") : t("ask.connect_obs")}</p>
          ) : (
            <select autoFocus aria-label={title} value={choice} onChange={(e) => { setChoice(e.target.value); setError(null); }}>
              <option value="">{t("ask.choose")}</option>
              {options.map((o) => <option key={o.id} value={o.id}>{o.label}</option>)}
            </select>
          )
        )}
        {error && <small className="warn-text">{error}</small>}
        <div className="row">
          <Button variant="primary" disabled={needsObs || empty} onClick={confirm}>{t("library.add")}</Button>
          <Button onClick={onClose}>{t("common.cancel")}</Button>
        </div>
      </div>
    </div>
  );
}
