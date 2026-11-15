import { describe, expect, test } from "vitest";
import { keyFace, PLAIN_FACE } from "../src/look";

describe("keyFace", () => {
  test("no colour: dark gradient, light icon, faint outline", () => {
    expect(keyFace(undefined)).toEqual({
      background: "linear-gradient(180deg, #1c1c22 0%, #0d0d10 100%)",
      ink: "#f2f2f2",
      border: "rgba(255,255,255,0.10)",
    });
    expect(keyFace(undefined)).toBe(PLAIN_FACE);
  });

  test("a colour: 12% lighter on top, 18% darker at the bottom, white icon", () => {
    // #2563eb = 37,99,235: +12% towards white, -18% towards black
    expect(keyFace("#2563eb")).toEqual({
      background: "linear-gradient(180deg, rgb(63,118,237) 0%, rgb(30,81,193) 100%)",
      ink: "#ffffff",
      border: "rgba(255,255,255,0.18)",
    });
  });

  test("short hex works, garbage falls back to the plain face", () => {
    expect(keyFace("#fff").background).toBe("linear-gradient(180deg, rgb(255,255,255) 0%, rgb(209,209,209) 100%)");
    expect(keyFace("red")).toBe(PLAIN_FACE);
  });
});
