// T27.F.1: the island panel background must stay >= 0.92 opaque; the resting dim never touches the panel itself.
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const css = readFileSync(new URL("../src/island/island.css", import.meta.url), "utf8");

describe("island panel opacity (T27.F.1)", () => {
  it("theme backgrounds have alpha >= 0.92 (acrylic variant excluded)", () => {
    const base = css.split("T26.F.3: acrylic")[0];
    const alphas = [...base.matchAll(/--island-bg:\s*rgba\([^)]*,\s*([\d.]+)\)/g)].map((m) => Number(m[1]));
    expect(alphas.length).toBe(2);
    for (const a of alphas) expect(a).toBeGreaterThanOrEqual(0.92);
  });

  it("no opacity declaration on a panel-level compact rule", () => {
    const rules = [...css.matchAll(/(^|\n)(\.island-compact[^{,]*?)\s*\{([^}]*)\}/g)];
    for (const [, , selector, body] of rules) {
      const panelLevel = /^\.island-compact(\.[\w-]+|:[\w-]+)*$/.test(selector.trim());
      if (panelLevel) expect(body, selector).not.toMatch(/(^|;|\s)opacity\s*:/);
    }
  });
});
