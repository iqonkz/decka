import { t, type TranslationKey } from "@decka/shared";

export type IntegrationCategory = "streaming" | "notifications" | "data" | "developers";
export type IntegrationFilterChip = "all" | "connected" | IntegrationCategory;

export interface IntegrationDef {
  id: string;
  name: string;
  titleKey: TranslationKey;
  descKey: TranslationKey;
  category: IntegrationCategory;
  icon: string;
  connected: boolean;
  toggle?: boolean;
  keywords?: string[];
}

export function filterIntegrations(
  items: IntegrationDef[],
  chip: IntegrationFilterChip,
  query: string,
  tFunc: (key: TranslationKey) => string = t,
): IntegrationDef[] {
  const q = query.trim().toLowerCase();
  return items.filter((item) => {
    if (chip === "connected") {
      if (!item.connected) return false;
    } else if (chip !== "all") {
      if (item.category !== chip) return false;
    }

    if (q) {
      const name = item.name.toLowerCase();
      let localizedTitle = "";
      let localizedDesc = "";
      try {
        localizedTitle = tFunc(item.titleKey).toLowerCase();
      } catch {
        // ignore
      }
      try {
        localizedDesc = tFunc(item.descKey).toLowerCase();
      } catch {
        // ignore
      }
      const keywords = (item.keywords ?? []).map((k) => k.toLowerCase());

      const match =
        name.includes(q) ||
        localizedTitle.includes(q) ||
        localizedDesc.includes(q) ||
        keywords.some((k) => k.includes(q));

      if (!match) return false;
    }

    return true;
  });
}
