import { describe, expect, test } from "vitest";
import { contrast } from "../src/theme/contrast";
import { materialKeyVars, materialRoles, materialRootVars } from "../src/theme/material";

const HEX = /^#[0-9a-f]{6}$/;

describe("Material scheme (DESIGN.md 1.3)", () => {
  test("every role of the default accent is a valid #rrggbb", () => {
    const roles = materialRoles("#7c5cff");
    for (const [name, value] of Object.entries(roles)) expect(value, name).toMatch(HEX);
  });

  test("the on-container colour is readable on the container (>= 4.5)", () => {
    for (const seed of ["#7c5cff", "#16a34a", "#ef4444", "#eab308", "#3b82f6", "#ec4899"]) {
      const r = materialRoles(seed);
      expect(contrast(r.onPrimaryContainer, r.primaryContainer), seed).toBeGreaterThanOrEqual(4.5);
      expect(contrast(r.onSurface, r.surface), seed).toBeGreaterThanOrEqual(4.5);
      expect(contrast(r.onSurfaceVariant, r.surfaceContainerHigh), seed).toBeGreaterThanOrEqual(4.5);
    }
  });

  test("the scheme is dark: surfaces are darker than their text", () => {
    const r = materialRoles("#7c5cff");
    expect(contrast(r.surface, "#000000")).toBeLessThan(contrast(r.surface, "#ffffff"));
  });

  test("schemes are cached by colour and an invalid colour falls back to the default accent", () => {
    expect(materialRoles("#16a34a")).toBe(materialRoles("#16A34A"));
    expect(materialRoles("nonsense")).toBe(materialRoles("#7c5cff"));
  });

  test("a different accent gives a different primary container", () => {
    expect(materialRoles("#7c5cff").primaryContainer).not.toBe(materialRoles("#16a34a").primaryContainer);
  });

  test("CSS variables are named for the theme sheet", () => {
    expect(Object.keys(materialRootVars("#7c5cff"))).toContain("--m-surface-container-high");
    expect(materialKeyVars("#16a34a")).toEqual({ "--mk-container": materialRoles("#16a34a").primaryContainer, "--mk-on": materialRoles("#16a34a").onPrimaryContainer });
  });
});

describe("contrast", () => {
  test("black on white is 21, a colour on itself is 1", () => {
    expect(contrast("#000000", "#ffffff")).toBeCloseTo(21, 5);
    expect(contrast("#7c5cff", "#7c5cff")).toBeCloseTo(1, 5);
  });
});
