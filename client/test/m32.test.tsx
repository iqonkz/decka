import { renderToStaticMarkup } from "react-dom/server";
import { beforeEach, describe, expect, test } from "vitest";
import { setLanguage } from "@decka/shared";
import PhoneMenu from "../src/menu/PhoneMenu";
import { resetState } from "../src/store/store";

describe("M32 T32.1b PhoneMenu PCs list", () => {
  beforeEach(() => {
    resetState();
    setLanguage("ru");
    if (typeof (globalThis as { location?: unknown }).location === "undefined") {
      (globalThis as { location: { host: string } }).location = { host: "192.168.1.20:9247" };
    }
  });

  test("renders PCs list with online/offline status, current marker, and Add PC action", () => {
    const html = renderToStaticMarkup(
      <PhoneMenu
        reduced={false}
        onToggleReduced={() => {}}
        haptics
        onToggleHaptics={() => {}}
        onUnpair={() => {}}
        onClose={() => {}}
        pcs={[
          { id: "pc-1", name: "WORKSTATION", address: "192.168.1.20", online: true, current: true },
          { id: "pc-2", name: "HOME-PC", address: "192.168.1.45", online: true },
          { id: "pc-3", name: "LAPTOP", address: "192.168.1.50", online: false },
        ]}
      />,
    );
    expect(html).toContain("ПК");
    expect(html).toContain("WORKSTATION");
    expect(html).toContain("HOME-PC");
    expect(html).toContain("LAPTOP");
    expect(html).toContain("В сети");
    expect(html).toContain("Не в сети");
    expect(html).toContain("Текущий");
    expect(html).toContain("Добавить ПК");
  });
});
