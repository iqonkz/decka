import { describe, expect, test } from "vitest";
import { effectivePrivacy } from "../src/notifPrivacy";

describe("effectivePrivacy", () => {
  test("messengers are app-only by default", () => {
    for (const app of ["Telegram Desktop", "WhatsApp", "Microsoft Teams", "Outlook", "Mail", "Discord"]) expect(effectivePrivacy(app, undefined), app).toBe("app");
    expect(effectivePrivacy("Calendar", undefined)).toBe("full");
  });
  test("the owner's choice for a program wins, then the default", () => {
    const p = { default: "full" as const, apps: { "telegram desktop": "full" as const, Calendar: "app" as const } };
    expect(effectivePrivacy("Telegram Desktop", p)).toBe("full");
    expect(effectivePrivacy("Calendar", p)).toBe("app");
    expect(effectivePrivacy("Spotify", { default: "app", apps: {} })).toBe("app");
  });
});
