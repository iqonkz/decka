import { describe, expect, test } from "vitest";
import { describeAction } from "../src/editor/describeAction";

describe("describeAction", () => {
  test("programs, sites, shortcuts", () => {
    expect(describeAction({ name: "Spotify", type: "launch", program: "C:\\Apps\\Spotify.exe" })).toBe("Launch Spotify");
    expect(describeAction({ name: "x", type: "open_url", url: "https://www.github.com/iqonkz/decka" })).toBe("Open github.com");
    expect(describeAction({ name: "x", type: "hotkey", keys: ["ctrl", "shift", "m"] })).toBe("Press Ctrl+Shift+M");
  });
  test("media and system", () => {
    expect(describeAction({ name: "x", type: "media", op: "play_pause" })).toBe("Play / Pause");
    expect(describeAction({ name: "x", type: "media", op: "vol_down" })).toBe("Volume down");
    expect(describeAction({ name: "x", type: "system", op: "lock" })).toBe("Lock PC");
    expect(describeAction({ name: "x", type: "system", op: "sleep" })).toBe("Sleep");
  });
  test("no action", () => {
    expect(describeAction(undefined)).toBe("Nothing yet");
  });
});