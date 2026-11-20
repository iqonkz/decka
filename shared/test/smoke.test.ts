import { expect, test } from "vitest";
import { PROTOCOL_VERSION } from "../src/index";

test("protocol version is 2", () => {
  expect(PROTOCOL_VERSION).toBe(2);
});