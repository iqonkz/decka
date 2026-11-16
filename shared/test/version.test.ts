import { describe, expect, test } from "vitest";
import { sameMinor } from "../src";

describe("sameMinor (ARCHITECTURE.md 8a)", () => {
  test("the same MAJOR.MINOR is the same whatever the PATCH", () => {
    expect(sameMinor("0.3.0", "0.3.9")).toBe(true);
    expect(sameMinor("1.2.3", "1.2.3")).toBe(true);
    expect(sameMinor("1.2.3-beta.1", "1.2.0")).toBe(true);
    expect(sameMinor("v1.2.3", "1.2.0")).toBe(true);
  });
  test("a different MINOR or MAJOR is different", () => {
    expect(sameMinor("0.3.0", "0.4.0")).toBe(false);
    expect(sameMinor("0.9.0", "0.10.0")).toBe(false);
    expect(sameMinor("1.0.0", "2.0.0")).toBe(false);
  });
  test("a version that is unknown or unreadable raises no warning", () => {
    expect(sameMinor(undefined, "0.3.0")).toBe(true);
    expect(sameMinor("0.3.0", undefined)).toBe(true);
    expect(sameMinor("garbage", "0.3.0")).toBe(true);
  });
});
