import { renderToString } from "react-dom/server";
import { describe, expect, test } from "vitest";
import Button from "../src/ui/Button";
import ColorPicker from "../src/ui/ColorPicker";
import Segmented from "../src/ui/Segmented";
import Switch from "../src/ui/Switch";
import TimePickerPopover from "../src/ui/TimePickerPopover";

describe("keyboard focus ring and accessible controls (DESIGN 4.17)", () => {
  test("styles.css defines a 2 px --accent :focus-visible ring with 2 px offset for controls", async () => {
    const fsMod = "node:fs";
    const { readFileSync } = (await import(/* @vite-ignore */ fsMod)) as {
      readFileSync: (p: URL, enc: string) => string;
    };
    const css = readFileSync(new URL("../src/styles.css", import.meta.url), "utf8");
    expect(css).toContain("button:focus-visible");
    expect(css).toContain('[role="switch"]:focus-visible');
    expect(css).toContain('[role="radio"]:focus-visible');
    expect(css).toContain("outline: 2px solid var(--accent);");
    expect(css).toContain("outline-offset: 2px;");
  });

  test("Switch, Segmented, and Button render focusable native controls with proper ARIA roles", () => {
    const swHtml = renderToString(<Switch checked={true} onChange={() => {}} aria-label="Toggle" />);
    expect(swHtml).toContain("<button");
    expect(swHtml).toContain('role="switch"');
    expect(swHtml).toContain('aria-checked="true"');
    expect(swHtml).toContain('aria-label="Toggle"');
    expect(swHtml).not.toContain('tabindex="-1"');

    const segHtml = renderToString(
      <Segmented
        aria-label="Repeat"
        value="daily"
        onChange={() => {}}
        options={[
          { value: "once", label: "Once" },
          { value: "daily", label: "Daily" },
        ]}
      />,
    );
    expect(segHtml).toContain('role="radiogroup"');
    expect(segHtml).toContain('role="radio"');
    expect(segHtml).toContain('aria-label="Repeat"');
    expect(segHtml).toContain('aria-checked="true"');
    expect(segHtml).not.toContain('tabindex="-1"');

    const btnHtml = renderToString(
      <Button variant="primary" aria-label="Save action">
        Save
      </Button>,
    );
    expect(btnHtml).toContain("<button");
    expect(btnHtml).toContain('aria-label="Save action"');
    expect(btnHtml).not.toContain('tabindex="-1"');
  });

  test("ColorPicker and TimePickerPopover render localized aria-labels", () => {
    const cp = renderToString(<ColorPicker value="#7c5cff" onChange={() => {}} />);
    expect(cp).toContain('aria-label="Custom accent"');
    const tp = renderToString(<TimePickerPopover value="2026-11-25T10:00" onChange={() => {}} onClose={() => {}} />);
    expect(tp).toContain('aria-label="Previous month"');
    expect(tp).toContain('aria-label="Next month"');
  });
});
