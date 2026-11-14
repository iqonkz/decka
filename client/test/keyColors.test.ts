import { describe, expect, test } from "vitest";
import { keyColors, keySize } from "../src/deck/look";

describe("keySize", () => {
  test("70% of the smaller cell side, rounded", () => {
    expect(keySize(200, 100)).toBe(70);
    expect(keySize(100, 200)).toBe(70);
    expect(keySize(171, 171)).toBe(120);
  });
});

describe("keyColors", () => {
  test("no colour: transparent key, light icon", () => {
    expect(keyColors(undefined)).toEqual({
      background: "transparent",
      pressed: "rgba(255,255,255,0.10)",
      ink: "#e5e5e5",
      border: "rgba(255,255,255,0.16)",
    });
  });

  test("a bright colour tints the key at 16% (30% pressed) and colours the icon", () => {
    expect(keyColors("#2563eb")).toEqual({
      background: "rgba(37,99,235,0.16)",
      pressed: "rgba(37,99,235,0.30)",
      ink: "#2563eb",
      border: "rgba(37,99,235,0.5)",
    });
  });

  test("a colour darker than luminance 64 keeps the icon light", () => {
    expect(keyColors("#202020").ink).toBe("#e5e5e5");
    expect(keyColors("#202020").background).toBe("rgba(32,32,32,0.16)");
  });

  test("short hex works, garbage falls back to no colour", () => {
    expect(keyColors("#fff").background).toBe("rgba(255,255,255,0.16)");
    expect(keyColors("red").background).toBe("transparent");
  });
});
