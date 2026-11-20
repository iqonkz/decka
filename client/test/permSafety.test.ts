import { describe, expect, test } from "vitest";
import type { PermRequest } from "@decka/shared";
import { PERM_HOLD_MS, offersAlways } from "../src/deck/PermSheet";

const base: PermRequest = { reqId: "0123456789abcdef", provider: "claude", tool: "Bash", summary: "git status", project: "p", sessionId: "abcdef12", expiresAt: 1 };

describe("Always allow on the phone (DATA_MODEL.md 11.3)", () => {
  test("offered for a command, a project and a host", () => {
    for (const kind of ["command", "project", "host"] as const) expect(offersAlways({ ...base, always: { kind, value: "x" } }), kind).toBe(true);
  });
  test("never for a whole tool, a dangerous command, or when nothing is safe to remember", () => {
    expect(offersAlways({ ...base, always: { kind: "tool", value: "mcp__x" } })).toBe(false);
    expect(offersAlways({ ...base, always: { kind: "command", value: "rm" }, danger: true })).toBe(false);
    expect(offersAlways(base)).toBe(false);
  });
  test("a dangerous command is allowed by holding for one second", () => {
    expect(PERM_HOLD_MS).toBe(1000);
  });
});
