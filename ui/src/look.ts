import { t, type ButtonItem, type ButtonLook, type VarValue } from "@decka/shared";

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
