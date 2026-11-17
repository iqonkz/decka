import { beforeEach, describe, expect, test } from "vitest";
import type { PatchMsg, PermRequest, WelcomeMsg } from "@decka/shared";
import welcomeFixture from "@decka/shared/fixtures/s2c-welcome.json";
import { applyPatch, applyWelcome, currentPerm, getState, markPermAnswered, onAttention, reopenPerm, resetState } from "../src/store/store";

const welcome = welcomeFixture as unknown as WelcomeMsg;
const patch = (rev: number, value: unknown) => ({ t: "patch", rev, section: "ai", value }) as unknown as PatchMsg;
const request = (reqId: string): PermRequest => ({ reqId, provider: "codex", tool: "Bash", summary: "ls", project: "decka", sessionId: "abcdef12", expiresAt: 9_999_999_999_999 });
const ai = (...ids: string[]) => ({ claude: null, codex: null, sessions: [], pending: ids.map(request) });

beforeEach(() => {
  resetState();
  applyWelcome(welcome);
});

describe("the permission sheet", () => {
  test("shows the oldest question, and an answer takes it away at once", () => {
    applyPatch(patch(43, ai("a".repeat(16), "b".repeat(16))));
    expect(currentPerm(getState())?.request.reqId).toBe("a".repeat(16));
    expect(currentPerm(getState())?.total).toBe(2);
    markPermAnswered("a".repeat(16)); // the patch has not come yet
    expect(currentPerm(getState())?.request.reqId).toBe("b".repeat(16));
    markPermAnswered("b".repeat(16));
    expect(currentPerm(getState())).toBeNull();
  });

  test("a question that leaves pending closes the sheet and is forgotten", () => {
    applyPatch(patch(43, ai("a".repeat(16))));
    expect(currentPerm(getState())).not.toBeNull();
    applyPatch(patch(44, ai())); // answered on the PC, or the wait ran out
    expect(currentPerm(getState())).toBeNull();
    markPermAnswered("c".repeat(16));
    applyPatch(patch(45, ai("a".repeat(16))));
    expect(getState().permClosed).toEqual([]);
    expect(currentPerm(getState())?.request.reqId).toBe("a".repeat(16));
  });

  test("a new question wakes the screen; the badge brings put-away questions back", () => {
    const levels: string[] = [];
    const off = onAttention((l) => levels.push(l));
    applyPatch(patch(43, ai("a".repeat(16))));
    applyPatch(patch(44, ai("a".repeat(16)))); // the same question again: no new wake-up
    off();
    expect(levels).toEqual(["warning"]);
    markPermAnswered("a".repeat(16));
    reopenPerm();
    expect(currentPerm(getState())).not.toBeNull();
  });

  test("a section null (both integrations off) has no sheet", () => {
    applyPatch(patch(43, ai("a".repeat(16))));
    applyPatch(patch(44, null));
    expect(currentPerm(getState())).toBeNull();
  });
});
