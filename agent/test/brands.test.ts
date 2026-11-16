import { describe, expect, test } from "vitest";
import { searchBrands, type Brand } from "../src/tabs/brands";

const list: Brand[] = [
  { title: "Adobe", slug: "adobe", hex: "FF0000" },
  { title: "Discord", slug: "discord", hex: "5865F2" },
  { title: "Steam", slug: "steam", hex: "000000" },
  { title: "Steamworks", slug: "steamworks", hex: "000000" },
];

describe("searchBrands", () => {
  test("an empty query puts the popular ones first", () => {
    expect(searchBrands(list, "").map((b) => b.slug).slice(0, 2)).toEqual(["discord", "steam"]);
  });
  test("matches words in the title, starts-with first", () => {
    expect(searchBrands(list, "ste").map((b) => b.slug)).toEqual(["steam", "steamworks"]);
    expect(searchBrands(list, "nothing")).toEqual([]);
  });
});
