import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, test } from "vitest";
import { setLanguage, type LayoutItem } from "@decka/shared";
import { DeckEnvContext, EMPTY_DATA, type DeckData, type DeckEnv } from "../src/deckEnv";
import { localizedButtonLabel } from "../src/look";
import { DeckMetricsContext } from "../src/metricsContext";
import PageDots from "../src/PageDots";
import Skeleton from "../src/Skeleton";
import Tile from "../src/Tile";
import { averageRgbFromPixels, COVER_MAX_LIGHTNESS, coverGradient, darkenCoverColor } from "../src/widgets/coverColor";
import { containerDensity, hasWidgetHeader, widgetDensityByCells } from "../src/widgets/density";

const NOOP = () => {};

function renderTile(item: LayoutItem, data: Partial<DeckData> = {}) {
  const env: DeckEnv = {
    ...EMPTY_DATA,
    online: true,
    ...data,
    onPress: () => Promise.resolve(),
    onPressError: NOOP,
    onNavigate: NOOP,
    onDismiss: NOOP,
  };
  return renderToStaticMarkup(
    <DeckEnvContext.Provider value={env}>
      <DeckMetricsContext.Provider value={{ side: 100, gap: 4 }}>
        <Tile item={item} />
      </DeckMetricsContext.Provider>
    </DeckEnvContext.Provider>,
  );
}

const at = (kind: string, w: number, h: number, extra: object = {}) =>
  ({ id: "w1", kind, x: 0, y: 0, w, h, ...extra }) as LayoutItem;

describe("widget density S/M/L", () => {
  test("containerDensity selects S, M, L from container dimensions", () => {
    expect(containerDensity(90, 90)).toBe("S");
    expect(containerDensity(200, 90)).toBe("S");
    expect(containerDensity(140, 140)).toBe("M");
    expect(containerDensity(220, 90)).toBe("M");
    expect(containerDensity(220, 140)).toBe("L");
    expect(containerDensity(140, 220)).toBe("L");
  });

  test("widgetDensityByCells classifies grid sizes cleanly", () => {
    expect(widgetDensityByCells(1, 1)).toBe("S");
    expect(widgetDensityByCells(2, 1)).toBe("S");
    expect(widgetDensityByCells(2, 2)).toBe("M");
    expect(widgetDensityByCells(3, 1)).toBe("M");
    expect(widgetDensityByCells(3, 2)).toBe("L");
    expect(widgetDensityByCells(4, 2)).toBe("L");
  });

  test("hasWidgetHeader renders on h >= 2 or w >= 3 widget panels only", () => {
    expect(hasWidgetHeader({ kind: "weather", w: 1, h: 1 })).toBe(false);
    expect(hasWidgetHeader({ kind: "weather", w: 2, h: 1 })).toBe(false);
    expect(hasWidgetHeader({ kind: "weather", w: 2, h: 2 })).toBe(true);
    expect(hasWidgetHeader({ kind: "media", w: 3, h: 1 })).toBe(true);
    expect(hasWidgetHeader({ kind: "button", w: 2, h: 2 })).toBe(false);
    expect(hasWidgetHeader({ kind: "voice", w: 2, h: 2 })).toBe(false);
    expect(hasWidgetHeader({ kind: "context", w: 2, h: 2 })).toBe(false);
  });

  test("Tile renders .tile-head only when h >= 2 or w >= 3", () => {
    expect(renderTile(at("weather", 1, 1))).not.toContain('class="tile-head"');
    expect(renderTile(at("volume", 2, 1))).not.toContain('class="tile-head"');
    expect(renderTile(at("weather", 2, 2))).toContain('class="tile-head"');
    expect(renderTile(at("volume", 3, 1))).toContain('class="tile-head"');
  });
});

describe("cover color sampling and darkening", () => {
  test("darkenCoverColor caps bright colors at lightness <= 30%", () => {
    const white = darkenCoverColor([255, 255, 255]);
    expect(white.lightness).toBeLessThanOrEqual(COVER_MAX_LIGHTNESS + 0.01);

    const yellow = darkenCoverColor([255, 230, 80]);
    expect(yellow.lightness).toBeLessThanOrEqual(COVER_MAX_LIGHTNESS + 0.01);
    expect(yellow.hex).toMatch(/^#[0-9a-f]{6}$/);

    const alreadyDark = darkenCoverColor([18, 22, 40]);
    expect(alreadyDark.lightness).toBeLessThanOrEqual(COVER_MAX_LIGHTNESS);
  });

  test("averageRgbFromPixels ignores transparent pixels and averages opaque ones", () => {
    const pixels = [
      255, 0, 0, 0, // fully transparent red (ignored)
      100, 150, 200, 255, // opaque
      140, 190, 240, 255, // opaque
    ];
    expect(averageRgbFromPixels(pixels)).toEqual([120, 170, 220]);
    expect(averageRgbFromPixels([0, 0, 0, 0])).toEqual([32, 34, 46]);
  });

  test("coverGradient outputs linear-gradient CSS", () => {
    const css = coverGradient([230, 120, 40]);
    expect(css.startsWith("linear-gradient(145deg, #")).toBe(true);
  });
});

describe("unified empty states and skeleton", () => {
  test("all data widgets use .widget-empty instead of dash rings when empty", () => {
    for (const item of [
      at("weather", 2, 2),
      at("sys", 2, 2),
      at("ai", 2, 2, { show: "both" }),
      at("media", 2, 2),
      at("obs", 2, 2),
      at("twitch", 2, 2),
      at("youtube", 2, 2),
      at("battery", 2, 2),
      at("mail", 2, 1),
      at("reminders", 2, 1),
      at("list", 2, 1),
      at("notifications", 2, 1),
      at("statuses", 2, 1),
    ]) {
      const html = renderTile(item, {});
      expect(html, `${item.kind} empty`).toContain('class="widget-empty"');
      expect(html, `${item.kind} no dash ring`).not.toContain('class="ai-ring-num">—');
    }
  });

  test("Skeleton applies radius styling cleanly", () => {
    const html = renderToStaticMarkup(<Skeleton radius={18} />);
    expect(html).toContain('class="skeleton"');
    expect(html).toContain("18px");
  });

  test("T31.7b: catalog button labels localize by actionId and inside badges stay inset 8 px", () => {
    setLanguage("ru");
    const html = renderTile({
      id: "b-lock",
      kind: "button",
      x: 0,
      y: 0,
      w: 1,
      h: 1,
      actionId: "lock-pc",
      label: "Lock",
      navigate: "second",
      icon: "lucide:lock",
    });
    expect(html).toContain("Блок");
    expect(html).toContain("top:8px;right:8px;font-size:14px");
    setLanguage("en");
  });

  test("T31.R: the user's own captions never change, and a key without a caption stays without one", () => {
    setLanguage("ru");
    for (const actionId of ["lock-pc", "mute", "media-next", "obs-rec", "my-program"]) {
      for (const own of ["Мой замок", "Anna's key", "Lock the door", "Стрим!"]) expect(localizedButtonLabel({ actionId }, own), `${actionId} ${own}`).toBe(own);
      expect(localizedButtonLabel({ actionId }, undefined), actionId).toBeUndefined();
      expect(localizedButtonLabel({ actionId }, ""), actionId).toBe("");
    }
    expect(localizedButtonLabel({ actionId: "lock-pc" }, "Lock")).toBe("Блок");
    expect(localizedButtonLabel({ actionId: undefined }, "Lock")).toBe("Lock");
    setLanguage("en");
  });

  test("T31.7b: PageDots renders interactive 32 px touch buttons when onSelect is provided", () => {
    const html = renderToStaticMarkup(<PageDots count={3} active={1} onSelect={() => {}} />);
    expect(html).toContain("page-dot-btn");
    expect(html).toContain('class="dot active"');
    expect(html).toContain('aria-current="page"');
  });
});
