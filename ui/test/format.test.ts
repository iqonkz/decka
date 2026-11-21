import { describe, expect, test } from "vitest";
import { formatClock, formatDate, formatRate, formatWeekday, hasDateData, percent, type IntlLike } from "../src/widgets/format";

/** An `Intl` that lists every language as supported but has no month names (the WebView case for `kk`). */
const bareIntl = {
  DateTimeFormat: Object.assign(
    function () {
      return { format: () => "M11 21, Sat" };
    },
    { supportedLocalesOf: (l: string) => [l] },
  ),
} as unknown as IntlLike;
const emptyIntl = { DateTimeFormat: Object.assign(function () {}, { supportedLocalesOf: () => [] }) } as unknown as IntlLike;

describe("dates without Intl data", () => {
  const d = new Date(2026, 10, 21); // Saturday
  test("detects missing data: no supported locales, or `M11` in the output", () => {
    expect(hasDateData("kk", bareIntl)).toBe(false);
    expect(hasDateData("kk", emptyIntl)).toBe(false);
    expect(hasDateData("en")).toBe(true);
  });
  test("kk comes from the dictionary: `21 қараша, сб`", () => {
    expect(formatDate(d, "kk", bareIntl)).toBe("21 қараша, сб");
    expect(formatWeekday(d, "kk", emptyIntl)).toBe("сб");
  });
  test("other languages follow the same rule", () => {
    expect(formatDate(d, "ru", bareIntl)).toBe("сб, 21 ноября");
    expect(formatDate(d, "en", bareIntl)).toBe("Sat, 21 November");
  });
});

describe("formatRate", () => {
  test("Kbps below 1000, Mbps with one decimal from 1000", () => {
    expect(formatRate(0)).toBe("0 Kbps");
    expect(formatRate(80)).toBe("80 Kbps");
    expect(formatRate(999)).toBe("999 Kbps");
    expect(formatRate(1000)).toBe("1.0 Mbps");
    expect(formatRate(12_340)).toBe("12.3 Mbps");
  });
});

describe("percent", () => {
  test("rounds to an integer and survives a zero total", () => {
    expect(percent(1, 3)).toBe(33);
    expect(percent(2, 3)).toBe(67);
    expect(percent(5, 0)).toBe(0);
    expect(percent(210, 476)).toBe(44);
  });
});

describe("formatClock", () => {
  test("24-hour with leading zeros, optional seconds", () => {
    const d = new Date(2026, 10, 14, 7, 5, 9);
    expect(formatClock(d, false)).toBe("07:05");
    expect(formatClock(d, true)).toBe("07:05:09");
    expect(formatClock(new Date(2026, 10, 14, 23, 59, 0), false)).toBe("23:59");
  });
});
