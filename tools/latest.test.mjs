import assert from "node:assert/strict";
import { test } from "node:test";
import { isNewer, latestJson, parseLatest } from "./lib/latest-json.mjs";

const input = { version: "0.11.0", notes: "Rings, one-click update.\n", signature: "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZQo=\n", installerName: "Decka-Setup-0.11.0.exe", pubDate: "2026-11-21T12:00:00Z" };

test("latest.json has what tauri-plugin-updater reads", () => {
  const json = latestJson(input);
  assert.equal(json.version, "0.11.0");
  assert.equal(json.pub_date, "2026-11-21T12:00:00Z");
  assert.equal(json.notes, "Rings, one-click update.");
  assert.equal(json.platforms["windows-x86_64"].url, "https://github.com/iqonkz/decka/releases/download/v0.11.0/Decka-Setup-0.11.0.exe");
  assert.equal(json.platforms["windows-x86_64"].signature, "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZQo=");
});

test("a manifest is read back; a broken one is refused", () => {
  const text = JSON.stringify(latestJson(input));
  assert.deepEqual(parseLatest(text), { version: "0.11.0", notes: "Rings, one-click update.", signature: "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZQo=", url: "https://github.com/iqonkz/decka/releases/download/v0.11.0/Decka-Setup-0.11.0.exe" });
  assert.equal(parseLatest("not json"), null);
  assert.equal(parseLatest(JSON.stringify({ version: "0.11.0", platforms: {} })), null);
  const foreign = latestJson(input);
  foreign.platforms["windows-x86_64"].url = "https://example.com/Decka.exe";
  assert.equal(parseLatest(JSON.stringify(foreign)), null);
});

test("no signature or a pre-release version: nothing is written", () => {
  assert.throws(() => latestJson({ ...input, signature: " " }), /signature is empty/);
  assert.throws(() => latestJson({ ...input, version: "0.11.0-beta.1" }), /stable/);
});

test("version comparison: only a newer stable version counts", () => {
  assert.equal(isNewer("0.11.0", "0.10.2"), true);
  assert.equal(isNewer("0.10.10", "0.10.9"), true);
  assert.equal(isNewer("0.10.2", "0.10.2"), false);
  assert.equal(isNewer("0.9.9", "0.10.0"), false);
  assert.equal(isNewer("1.0.0-beta.1", "0.10.0"), false);
});
