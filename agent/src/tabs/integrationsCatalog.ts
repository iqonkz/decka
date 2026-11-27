import { t, type ConfigFile, type TranslationKey } from "@decka/shared";

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

/** What the cards need that is not in `config.json`: whether a key sends a webhook, whether the deck shows device batteries. */
export interface IntegrationUse {
  webhooks: boolean;
  batteryWidget: boolean;
}

export function buildIntegrationsList(
  config: ConfigFile,
  checksCount: number,
  lyricsEnabled: boolean,
  use: IntegrationUse = { webhooks: false, batteryWidget: false },
): IntegrationDef[] {
  return [
    {
      id: "obs",
      name: "OBS Studio",
      titleKey: "integ.catalog_title",
      descKey: "integ.item_obs_desc",
      category: "streaming",
      icon: "broadcast",
      connected: Boolean(config.obs?.enabled),
      keywords: ["obs", "stream", "scenes", "recording", "broadcast"],
    },
    {
      id: "twitch",
      name: "Twitch",
      titleKey: "integ.catalog_title",
      descKey: "integ.item_twitch_desc",
      category: "streaming",
      icon: "chat-circle-text",
      connected: Boolean(config.twitch?.enabled),
      keywords: ["twitch", "stream", "chat", "live"],
    },
    {
      id: "youtube",
      name: "YouTube",
      titleKey: "integ.catalog_title",
      descKey: "integ.item_youtube_desc",
      category: "streaming",
      icon: "play-circle",
      connected: Boolean(config.youtube?.enabled),
      keywords: ["youtube", "video", "broadcast", "subscribers"],
    },
    {
      id: "mail",
      name: t("integ.item_mail_name"),
      titleKey: "integ.catalog_title",
      descKey: "integ.item_mail_desc",
      category: "notifications",
      icon: "envelope",
      connected: Boolean(config.mail?.accounts && config.mail.accounts.length > 0),
      keywords: ["email", "mail", "gmail", "imap", "unread"],
    },
    {
      id: "winnotif",
      name: t("winnotif.title"),
      titleKey: "winnotif.title",
      descKey: "integ.item_winnotif_desc",
      category: "notifications",
      icon: "bell",
      connected: Boolean(config.windowsNotifications?.enabled),
      keywords: ["windows", "notifications", "alerts", "toasts"],
    },
    {
      id: "weather",
      name: t("weather.title"),
      titleKey: "weather.title",
      descKey: "integ.item_weather_desc",
      category: "data",
      icon: "cloud-sun",
      connected: Boolean(config.weather?.enabled && config.weather?.place),
      keywords: ["weather", "temperature", "forecast", "city"],
    },
    {
      id: "checks",
      name: t("integ.checks"),
      titleKey: "integ.checks",
      descKey: "integ.item_checks_desc",
      category: "data",
      icon: "pulse",
      connected: Boolean(checksCount > 0),
      keywords: ["checks", "http", "uptime", "server", "ping"],
    },
    {
      id: "ai",
      name: t("integ.item_ai_name"),
      titleKey: "integ.catalog_title",
      descKey: "integ.item_ai_desc",
      category: "data",
      icon: "brain",
      connected: Boolean(config.ai?.claude?.enabled || config.ai?.codex?.enabled),
      keywords: ["ai", "claude", "codex", "llm", "limits"],
    },
    {
      id: "lyrics",
      name: t("integ.item_lyrics_name"),
      titleKey: "integ.item_lyrics_name",
      descKey: "integ.item_lyrics_desc",
      category: "data",
      icon: "music-notes",
      connected: lyricsEnabled,
      toggle: true,
      keywords: ["lyrics", "lrclib", "songs", "music", "synced", "текст", "песни"],
    },
    {
      id: "bluetooth",
      name: t("m29.integ_bt_name"),
      titleKey: "m29.integ_bt_name",
      descKey: "m29.integ_bt_desc",
      category: "data",
      icon: "bluetooth",
      connected: use.batteryWidget,
      keywords: ["bluetooth", "battery", "headphones", "device"],
    },
    {
      id: "webhook",
      name: t("m29.integ_webhook_name"),
      titleKey: "m29.integ_webhook_name",
      descKey: "m29.integ_webhook_desc",
      category: "developers",
      icon: "link",
      connected: use.webhooks,
      keywords: ["webhook", "http", "request", "post"],
    },
    {
      id: "calendar",
      name: t("m32.cal_name"),
      titleKey: "m32.cal_name",
      descKey: "m32.cal_desc",
      category: "data",
      icon: "calendar",
      connected: Boolean((config as { calendar?: { url?: string } }).calendar?.url),
      keywords: ["calendar", "ics", "google", "outlook", "meeting", "zoom", "meet", "календарь", "встречи"],
    },
    {
      id: "ha",
      name: t("m30.ha_name"),
      titleKey: "m30.ha_name",
      descKey: "m30.ha_desc",
      category: "data",
      icon: "house",
      connected: Boolean((config as { ha?: { url?: string } }).ha?.url),
      keywords: ["home assistant", "ha", "smart home", "light", "switch", "scene"],
    },
    {
      id: "api_token",
      name: t("integ.api_token"),
      titleKey: "integ.api_token",
      descKey: "integ.item_api_desc",
      category: "developers",
      icon: "key",
      connected: Boolean(config.apiToken),
      keywords: ["api", "token", "auth", "developer", "key", "curl", "powershell", "examples", "http"],
    },
  ];
}
