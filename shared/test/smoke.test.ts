import { expect, test } from "vitest";
import { PROTOCOL_VERSION } from "../src/index";

test("protocol version is 1", () => {
  expect(PROTOCOL_VERSION).toBe(1);
});