import { CATALOG, getLanguage, pickText, t, type ButtonItem, type ButtonLook, type CatalogItem, type LocalizedText, type VarValue } from "@decka/shared";

/** Captions the first layout and the templates wrote that differ from the catalog's. */
const TEMPLATE_CAPTIONS: Record<string, string> = { "tasks": "win.taskmgr", "play / pause": "media.play" };

const fold = (s: string) => s.trim().toLowerCase();
const texts = (caption: LocalizedText) => Object.values(caption).filter((text): text is string => typeof text === "string");
/** The captions of an item in the order they are trusted: of the key, of its states ("Mic off"), the full name. */
const captionTiers = (item: CatalogItem): LocalizedText[][] => [[item.label], Object.values(item.bind?.states ?? {}).flatMap((s) => (s.label ? [s.label] : [])), [item.name]];

/** A key made by voice keeps the first 12 characters of a long caption (T29.6). */
const wrote = (caption: LocalizedText, label: string) => texts(caption).some((text) => fold(text) === label || fold(Array.from(text).slice(0, 12).join("")) === label);

/** A layout older than `catalogId`: the label is exactly a caption of the catalog, and every item that has it translates it alike. */
function legacyCaption(label: string): string | undefined {
  const template = CATALOG.items.find((c) => c.id === TEMPLATE_CAPTIONS[label]);
  if (template) return pickText(template.label, getLanguage());
  for (let tier = 0; tier < 3; tier++) {
    const found = new Set<string>();
    for (const item of CATALOG.items) {
      for (const caption of captionTiers(item)[tier]!) if (texts(caption).some((text) => fold(text) === label)) found.add(pickText(caption, getLanguage()));
    }
    if (found.size > 0) return found.size === 1 ? [...found][0] : undefined;
  }
  return undefined;
}

/**
 * A caption the catalog wrote, in the language of the phone. A key made from the catalog (`catalogId`) follows it until the
 * user renames it; a layout without `catalogId` is matched by the text alone. Any other caption stays as it is.
 */
export function localizedButtonLabel(item: Pick<ButtonItem, "actionId" | "catalogId">, label: string | undefined): string | undefined {
  // A key without a caption stays without one: that is the user's choice too.
  if (!label) return label;
  if (item.catalogId !== undefined) {
    const made = CATALOG.items.find((c) => c.id === item.catalogId);
    const caption = made && captionTiers(made).flat().find((c) => wrote(c, fold(label)));
    return caption ? pickText(caption, getLanguage()) : label;
  }
  return item.actionId ? (legacyCaption(fold(label)) ?? label) : label;
}

/** Base look of the button with `bind.states[String(var)]` laid over it (DATA_MODEL.md 3.4). */
export function resolveLook(item: ButtonItem, vars: Record<string, VarValue> | undefined): ButtonLook {
  const base: ButtonLook = {};
  if (item.label !== undefined) base.label = item.label;
  if (item.icon !== undefined) base.icon = item.icon;
  if (item.color !== undefined) base.color = item.color;
  if (!item.bind) return base;

  const value = vars?.[item.bind.var];
  const key = value === undefined ? "false" : String(value);
  const override = item.bind.states[key];
  return override ? { ...base, ...definedOnly(override) } : base;
}

function definedOnly(look: ButtonLook): ButtonLook {
  const out: ButtonLook = {};
  if (look.label !== undefined) out.label = look.label;
  if (look.icon !== undefined) out.icon = look.icon;
  if (look.color !== undefined) out.color = look.color;
  return out;
}

/** Toast title for a failed press, by error code. */
export function pressErrorTitle(e: { code: string; message?: string }): string {
  switch (e.code) {
    case "UNKNOWN_ACTION":
      return t("press.unknown_action");
    case "ACTION_FAILED": {
      // T29.P: a webhook says its status code (also as a step of a sequence), or that nothing answered.
      const status = /^(?:Step \d+: )?HTTP (\d{3})$/.exec(e.message ?? "")?.[1];
      if (status !== undefined) return t("press.webhook_code", { code: status });
      if (e.message?.endsWith("The webhook did not answer")) return t("press.webhook_silent");
      return t("press.failed");
    }
    case "RATE_LIMITED":
      return t("press.rate_limited");
    case "TIMEOUT":
      return t("press.timeout");
    case "OFFLINE":
      return t("press.offline");
    default:
      return e.message || e.code;
  }
}
