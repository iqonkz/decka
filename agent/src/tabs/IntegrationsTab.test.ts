import { describe, expect, test } from "vitest";
import { filterIntegrations, type IntegrationDef } from "./integrationsCatalog";

const SAMPLE_INTEGRATIONS: IntegrationDef[] = [
  {
    id: "obs",
    name: "OBS Studio",
    titleKey: "integ.catalog_title",
    descKey: "integ.item_obs_desc",
    category: "streaming",
    icon: "broadcast",
    connected: true,
    keywords: ["obs", "stream", "scenes"],
  },
  {
    id: "twitch",
    name: "Twitch",
    titleKey: "integ.catalog_title",
    descKey: "integ.item_twitch_desc",
    category: "streaming",
    icon: "chat-circle-text",
    connected: false,
    keywords: ["twitch", "stream", "chat"],
  },
  {
    id: "mail",
    name: "Mail",
    titleKey: "integ.catalog_title",
    descKey: "integ.item_mail_desc",
    category: "notifications",
    icon: "envelope",
    connected: true,
    keywords: ["email", "imap", "unread"],
  },
  {
    id: "weather",
    name: "Weather",
    titleKey: "weather.title",
    descKey: "integ.item_weather_desc",
    category: "data",
    icon: "cloud-sun",
    connected: false,
    keywords: ["weather", "temperature", "forecast"],
  },
  {
    id: "lyrics",
    name: "Lyrics",
    titleKey: "integ.item_lyrics_name",
    descKey: "integ.item_lyrics_desc",
    category: "data",
    icon: "music-notes",
    connected: false,
    toggle: true,
    keywords: ["lyrics", "lrclib", "songs", "music"],
  },
  {
    id: "api_token",
    name: "API Token",
    titleKey: "integ.api_token",
    descKey: "integ.item_api_desc",
    category: "developers",
    icon: "key",
    connected: true,
    keywords: ["api", "token", "curl"],
  },
];

describe("filterIntegrations", () => {
  test("chip 'all' returns all items when query is empty", () => {
    const result = filterIntegrations(SAMPLE_INTEGRATIONS, "all", "");
    expect(result.map((i) => i.id)).toEqual(["obs", "twitch", "mail", "weather", "lyrics", "api_token"]);
  });

  test("chip 'connected' returns only connected integrations", () => {
    const result = filterIntegrations(SAMPLE_INTEGRATIONS, "connected", "");
    expect(result.map((i) => i.id)).toEqual(["obs", "mail", "api_token"]);
  });

  test("category chips filter correctly", () => {
    const streaming = filterIntegrations(SAMPLE_INTEGRATIONS, "streaming", "");
    expect(streaming.map((i) => i.id)).toEqual(["obs", "twitch"]);

    const notifications = filterIntegrations(SAMPLE_INTEGRATIONS, "notifications", "");
    expect(notifications.map((i) => i.id)).toEqual(["mail"]);

    const data = filterIntegrations(SAMPLE_INTEGRATIONS, "data", "");
    expect(data.map((i) => i.id)).toEqual(["weather", "lyrics"]);

    const lrclib = filterIntegrations(SAMPLE_INTEGRATIONS, "data", "lrclib");
    expect(lrclib.map((i) => i.id)).toEqual(["lyrics"]);

    const developers = filterIntegrations(SAMPLE_INTEGRATIONS, "developers", "");
    expect(developers.map((i) => i.id)).toEqual(["api_token"]);
  });

  test("search query matches name and keywords case-insensitively", () => {
    const byName = filterIntegrations(SAMPLE_INTEGRATIONS, "all", "obs");
    expect(byName.map((i) => i.id)).toEqual(["obs"]);

    const byKeyword = filterIntegrations(SAMPLE_INTEGRATIONS, "all", "forecast");
    expect(byKeyword.map((i) => i.id)).toEqual(["weather"]);
  });

  test("combines category chip and search query", () => {
    const res1 = filterIntegrations(SAMPLE_INTEGRATIONS, "streaming", "twitch");
    expect(res1.map((i) => i.id)).toEqual(["twitch"]);

    const res2 = filterIntegrations(SAMPLE_INTEGRATIONS, "notifications", "twitch");
    expect(res2).toEqual([]);
  });

  test("returns empty array when query matches nothing", () => {
    const result = filterIntegrations(SAMPLE_INTEGRATIONS, "all", "nonexistent-xyz");
    expect(result).toEqual([]);
  });
});
