import { describe, expect, test } from "vitest";
import { normalizeCode, parseHostInput, parsePairUrl } from "../src/pairUrl";

describe("parsePairUrl", () => {
  test("URL with two hosts", () => {
    expect(parsePairUrl("http://192.168.1.5:9247/#c=K7M2QX9D&h=192.168.1.5,10.0.0.7&p=9247&n=DESKTOP%20MAIN")).toEqual({
      code: "K7M2QX9D",
      hosts: ["192.168.1.5", "10.0.0.7"],
      port: 9247,
      agentName: "DESKTOP MAIN",
    });
  });

  test("URL without h and p falls back to the URL host and port", () => {
    expect(parsePairUrl("http://192.168.1.5:9000/#c=K7M2QX9D")).toEqual({
      code: "K7M2QX9D",
      hosts: ["192.168.1.5"],
      port: 9000,
      agentName: "",
    });
    expect(parsePairUrl("http://pc.local/#c=K7M2QX9D")?.port).toBe(9247);
  });

  test("lowercase code with a hyphen", () => {
    expect(parsePairUrl("http://10.0.0.1:9247/#c=k7m2-qx9d")?.code).toBe("K7M2QX9D");
  });

  test("garbage returns null", () => {
    expect(parsePairUrl("hello")).toBeNull();
    expect(parsePairUrl("http://10.0.0.1:9247/")).toBeNull();
    expect(parsePairUrl("http://10.0.0.1:9247/#c=SHORT")).toBeNull();
    expect(parsePairUrl("http://10.0.0.1:9247/#c=K7M2QX90")).toBeNull(); // '0' is not in the alphabet
  });
});

describe("normalizeCode", () => {
  test("normalizes and rejects", () => {
    expect(normalizeCode(" k7m2-qx9d ")).toBe("K7M2QX9D");
    expect(normalizeCode("K7M2QX9")).toBeNull();
    expect(normalizeCode("IIIIIIII")).toBeNull();
  });
});

describe("parseHostInput", () => {
  test("host and host:port", () => {
    expect(parseHostInput("192.168.1.5")).toEqual({ host: "192.168.1.5", port: 9247 });
    expect(parseHostInput("192.168.1.5:9000")).toEqual({ host: "192.168.1.5", port: 9000 });
  });
  test("empty and invalid", () => {
    expect(parseHostInput("")).toBeNull();
    expect(parseHostInput("host:99999")).toBeNull();
  });
});
