import { createElement } from "react";
import { renderToString } from "react-dom/server";
import { describe, expect, test, vi } from "vitest";

vi.mock("../src/tabs/ThemeCards", () => ({
  default: () => createElement("div", { className: "theme-cards-mock" }),
}));

import FirstRun from "../src/tabs/FirstRun";

describe("PC onboarding restyle (DESIGN 4.7, 4.14-4.15)", () => {
  test("renders step 0 and step 2 with modern settings header, pills, and cards", () => {
    const step0 = renderToString(createElement(FirstRun, { onClose: () => {}, startStep: 0 }));
    expect(step0).toContain("wizard-topbar");
    expect(step0).toContain("wizard-step-pill active");
    expect(step0).toContain("Welcome to Decka");

    const step2 = renderToString(createElement(FirstRun, { onClose: () => {}, startStep: 2 }));
    expect(step2).toContain("settings-page-header");
    expect(step2).toContain("wizard-sets");
    expect(step2).toContain("settings-group-card");
  });
});
