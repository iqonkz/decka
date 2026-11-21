import { describe, expect, test } from "vitest";
import { formatHardware } from "../src/tabs/VoiceSection";

describe("VoiceSection formatHardware", () => {
  test("formats GPU with VRAM and RAM", () => {
    const text = formatHardware({ gpu: "NVIDIA GeForce RTX 2060 SUPER", vramGb: 8.0, ramGb: 15.8 }, "standard");
    expect(text).toContain("NVIDIA GeForce RTX 2060 SUPER (8 GB)");
    expect(text).toContain("15.8 GB RAM");
    expect(text).toContain("standard");
  });

  test("formats PC without GPU", () => {
    const text = formatHardware({ ramGb: 16.0 }, "standard");
    expect(text).toContain("16.0 GB RAM");
    expect(text).toContain("standard");
  });

  test("formats light model recommendation", () => {
    const text = formatHardware({ ramGb: 8.0 }, "light");
    expect(text).toContain("8.0 GB RAM");
    expect(text).toContain("light");
  });
});
