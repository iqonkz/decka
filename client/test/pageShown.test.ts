import { beforeEach, describe, expect, test } from "vitest";
import type { WelcomeMsg } from "@decka/shared";
import welcomeFixture from "@decka/shared/fixtures/s2c-welcome.json";
import { installPageShown } from "../src/app/pageShown";
import { applyWelcome, navigateManually, resetState, setState } from "../src/store/store";

const welcome = welcomeFixture as unknown as WelcomeMsg;
const first = welcome.state.layout.pages[0]!;
const two = { ...welcome, state: { ...welcome.state, layout: { ...welcome.state.layout, pages: [first, { ...first, id: "second", name: "Second" }] } } } as WelcomeMsg;

describe("page_shown", () => {
  beforeEach(resetState);

  test("goes after the welcome and when the page changes, once each", () => {
    const sent: string[] = [];
    const stop = installPageShown((id) => sent.push(id));
    applyWelcome(two);
    expect(sent).toEqual([first.id]);
    setState({ lastTouch: 5 }); // a change that turns no page
    navigateManually(1);
    navigateManually(1);
    expect(sent).toEqual([first.id, "second"]);
    stop();
  });

  test("is said again after a new connection, and never without one", () => {
    const sent: string[] = [];
    const stop = installPageShown((id) => sent.push(id));
    applyWelcome(two);
    setState({ conn: "connecting" });
    navigateManually(1);
    expect(sent).toEqual([first.id]);
    setState({ conn: "online" });
    expect(sent).toEqual([first.id, "second"]);
    stop();
  });
});
