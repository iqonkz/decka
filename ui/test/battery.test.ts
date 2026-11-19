import { describe, expect, test } from "vitest";
import { batteryIcon, chargeLevel } from "../src/widgets/batteryIcon";

describe("batteryIcon (DATA_MODEL.md 10.6)", () => {
  test("the PC is a laptop whatever its name", () => {
    expect(batteryIcon("pc", "Headset")).toBe("lucide:laptop");
  });
  test("a device gets its icon from words in its name", () => {
    expect(batteryIcon("bt", "WH-1000XM5 Headphones")).toBe("lucide:headphones");
    expect(batteryIcon("bt", "AirPods Pro")).toBe("lucide:headphones");
    expect(batteryIcon("bt", "MX Master Mouse")).toBe("lucide:mouse");
    expect(batteryIcon("bt", "Logitech Keyboard K380")).toBe("lucide:keyboard");
    expect(batteryIcon("bt", "Xbox Wireless Controller")).toBe("lucide:gamepad-2");
    expect(batteryIcon("bt", "Наушники Sony")).toBe("lucide:headphones");
  });
  test("anything else is plain bluetooth", () => {
    expect(batteryIcon("bt", "JBL Flip 6")).toBe("lucide:bluetooth");
    expect(batteryIcon("bt", "")).toBe("lucide:bluetooth");
  });
});

describe("chargeLevel", () => {
  test("below 20% warning, below 10% error", () => {
    expect(chargeLevel(80)).toBe("ok");
    expect(chargeLevel(20)).toBe("ok");
    expect(chargeLevel(19)).toBe("warning");
    expect(chargeLevel(10)).toBe("warning");
    expect(chargeLevel(9)).toBe("error");
    expect(chargeLevel(0)).toBe("error");
  });
});
