import test from "node:test";
import assert from "node:assert/strict";
import { check, rustParams, tsKeys } from "./check-ipc.mjs";

test("every command of commands.rs takes the keys that ipc.ts sends", () => {
  assert.deepEqual(check(), []);
});

test("the old nested voice_set signature is caught", () => {
  assert.deepEqual(rustParams("app: AppHandle, args: VoiceSetArgs"), ["args"]);
  assert.deepEqual(tsKeys("model?: string; hotkey?: string[]; keepLoaded?: boolean"), ["hotkey", "keepLoaded", "model"]);
});

test("flat snake_case parameters become camelCase keys", () => {
  assert.deepEqual(rustParams("app: AppHandle, keep_loaded: Option<bool>, hotkey: Option<Vec<String>>"), ["hotkey", "keepLoaded"]);
});
