import { describe, expect, test, vi } from "vitest";
import { renderToString } from "react-dom/server";
import Switch, { handleSwitchKeyDown } from "./Switch";

describe("Switch component", () => {
  test("renders with role='switch'", () => {
    const html = renderToString(<Switch checked={false} onChange={() => {}} />);
    expect(html).toContain('role="switch"');
  });

  test("renders with aria-checked matching checked state", () => {
    const htmlChecked = renderToString(<Switch checked={true} onChange={() => {}} />);
    expect(htmlChecked).toContain('aria-checked="true"');
    expect(htmlChecked).toContain("checked");

    const htmlUnchecked = renderToString(<Switch checked={false} onChange={() => {}} />);
    expect(htmlUnchecked).toContain('aria-checked="false"');
    expect(htmlUnchecked).not.toContain("settings-switch checked");
  });

  test("toggles on Space key press", () => {
    const onChange = vi.fn();
    const preventDefault = vi.fn();
    const handled = handleSwitchKeyDown({ key: " ", preventDefault }, false, onChange);
    expect(handled).toBe(true);
    expect(preventDefault).toHaveBeenCalled();
    expect(onChange).toHaveBeenCalledWith(true);
  });

  test("toggles from true to false on Space key", () => {
    const onChange = vi.fn();
    const preventDefault = vi.fn();
    const handled = handleSwitchKeyDown({ key: " ", preventDefault }, true, onChange);
    expect(handled).toBe(true);
    expect(preventDefault).toHaveBeenCalled();
    expect(onChange).toHaveBeenCalledWith(false);
  });

  test("does not toggle on Space when disabled", () => {
    const onChange = vi.fn();
    const preventDefault = vi.fn();
    const handled = handleSwitchKeyDown({ key: " ", preventDefault }, false, onChange, true);
    expect(handled).toBe(false);
    expect(preventDefault).not.toHaveBeenCalled();
    expect(onChange).not.toHaveBeenCalled();
  });
});
