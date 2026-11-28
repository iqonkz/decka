// T32.G: captions of keys.
import { describe, expect, test } from "vitest";
import { CATALOG, setLanguage } from "@decka/shared";
import { localizedButtonLabel } from "../src/look";

describe("captions of keys", () => {
  test("a key of the catalog without a caption is named by the catalog; an emptied one stays empty", () => {
    setLanguage("ru");
    const item = CATALOG.items.find((c) => c.id === "win.lock")!;
    expect(localizedButtonLabel({ actionId: "x", catalogId: item.id }, undefined)).toBe(item.label.ru);
    expect(localizedButtonLabel({ actionId: "x", catalogId: item.id }, "")).toBe("");
    expect(localizedButtonLabel({ actionId: "x" }, undefined)).toBeUndefined();
    setLanguage("en");
  });

  test("the words of a page template are not taken for captions of the catalog", () => {
    setLanguage("ru");
    expect(localizedButtonLabel({ actionId: "tpl-canva-group" }, "Group")).toBe("Group");
    expect(localizedButtonLabel({ actionId: "tpl-canva-lock" }, "Lock")).toBe("Lock");
    setLanguage("en");
  });
});
