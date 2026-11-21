import { describe, expect, test } from "vitest";
import type { CatalogItem } from "@decka/shared";
import { hintOf } from "../src/editor/libraryHint";

const item = (name: string, extra: Record<string, unknown>): CatalogItem => ({ id: "x", label: { en: name, ru: name }, name: { en: name, ru: name }, icon: "lucide:x", ...extra }) as unknown as CatalogItem;

describe("hintOf", () => {
  test("hotkey shows the chord, launch the exe, open_url the host", () => {
    expect(hintOf(item("Screenshot", { action: { type: "hotkey", keys: ["win", "shift", "s"] } }))).toBe("Win+Shift+S");
    expect(hintOf(item("Spotify", { action: { type: "launch", program: "C:\\Apps\\Spotify.exe" } }))).toBe("Spotify.exe");
    expect(hintOf(item("Repo", { action: { type: "open_url", url: "https://www.github.com/x" } }))).toBe("github.com");
  });
  test("media, audio, obs and system have no hint, whatever the op", () => {
    expect(hintOf(item("Play", { action: { type: "media", op: "play_pause" } }))).toBe("");
    expect(hintOf(item("Mic", { action: { type: "audio", op: "mic_mute" } }))).toBe("");
    expect(hintOf(item("Scene", { requires: "obs", action: { type: "obs", op: "scene" } }))).toBe("");
    expect(hintOf(item("Lock PC", { action: { type: "system", op: "lock" } }))).toBe("");
    expect(hintOf(item("Sleep", { action: { type: "system", op: "sleep" } }))).toBe("");
  });
  test("a widget shows its size; a hint equal to the name is dropped", () => {
    expect(hintOf(item("Clock", { widget: { w: 2, h: 1 } }))).toBe("2×1");
    expect(hintOf(item("github.com", { action: { type: "open_url", url: "https://github.com" } }))).toBe("");
  });
});
