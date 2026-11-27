// T29.P: the stored (protected) header values of a webhook never come to the form as text and survive an edit.
import { describe, expect, it } from "vitest";
import type { Action } from "@decka/shared";
import { fromDraft, HEADER_MASK, toDraft } from "../src/tabs/actionForm";

const hook: Action = { type: "http", name: "Hook", method: "GET", url: "https://example.com/hook", headers: { Authorization: "dpapi:QUJD", "X-Plain": "1" } };

describe("the headers of a webhook in the form", () => {
  it("shows a mask for a stored value and keeps that value on save", () => {
    const draft = toDraft("hook", hook);
    expect(draft.headersText).toBe(`Authorization: ${HEADER_MASK}\nX-Plain: 1`);
    expect(draft.headersText).not.toContain("QUJD");
    expect(fromDraft(draft)).toEqual({ action: hook });
  });

  it("takes a new value in place of the mask", () => {
    const draft = { ...toDraft("hook", hook), headersText: "Authorization: Bearer new" };
    expect(fromDraft(draft)).toEqual({ action: { ...hook, headers: { Authorization: "Bearer new" } } });
  });

  it("asks again when a masked line gets another name", () => {
    const draft = { ...toDraft("hook", hook), headersText: `X-Token: ${HEADER_MASK}` };
    expect(fromDraft(draft)).toEqual({ error: "Enter the value of “X-Token” again" });
  });
});
