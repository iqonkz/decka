import { describe, expect, it } from "vitest";
import { renderToString } from "react-dom/server";
import ColorPicker from "../src/ui/ColorPicker";
import RadioGroup from "../src/ui/RadioGroup";
import TimeField, { normalizeHHMM, parseHHMM } from "../src/ui/TimeField";

describe("T25.N shared controls", () => {
  it("parses and normalizes HH:mm strings", () => {
    expect(parseHHMM("22:30")).toEqual({ h: 22, m: 30 });
    expect(normalizeHHMM("7:05")).toBe("07:05");
    expect(normalizeHHMM("25:00")).toBeNull();
    expect(normalizeHHMM("12:60")).toBeNull();
  });

  it("renders TimeField, RadioGroup, and compact ColorPicker without native radio/time/color inputs", () => {
    const html = renderToString(
      <>
        <TimeField value="22:00" onChange={() => {}} aria-label="From" />
        <RadioGroup
          value="aurora"
          options={[
            { value: "aurora", label: "Aurora" },
            { value: "sunset", label: "Sunset" },
          ]}
          onChange={() => {}}
          aria-label="Wallpaper"
        />
        <ColorPicker compact value="#7c5cff" onChange={() => {}} aria-label="Color" />
      </>,
    );
    expect(html).not.toMatch(/type="(radio|time|color)"/);
    expect(html).toContain('role="radiogroup"');
    expect(html).toContain("22:00");
  });
});
