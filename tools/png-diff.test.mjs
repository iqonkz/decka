import assert from "node:assert/strict";
import { test } from "node:test";
import { comparePixels, decodePng, encodePng } from "./lib/png-diff.mjs";

function picture(width, height, paint) {
  const data = Buffer.alloc(width * height * 4, 255);
  for (let i = 0; i < width * height; i++) data.set(paint(i), i * 4);
  return { width, height, data };
}

test("a picture survives encoding and decoding", () => {
  const a = picture(7, 5, (i) => [i * 7, 255 - i, i % 3, 255]);
  const b = decodePng(encodePng(a));
  assert.equal(b.width, 7);
  assert.equal(b.height, 5);
  assert.deepEqual(b.data, a.data);
});

test("the share of changed pixels is counted; small noise is ignored", () => {
  const base = picture(20, 10, () => [100, 100, 100, 255]);
  const noisy = picture(20, 10, () => [108, 95, 100, 255]);
  assert.equal(comparePixels(base, noisy).ratio, 0);
  const changed = picture(20, 10, (i) => (i < 2 ? [0, 0, 0, 255] : [100, 100, 100, 255]));
  assert.equal(comparePixels(base, changed).ratio, 0.01);
});

test("another size differs completely", () => {
  const result = comparePixels(picture(4, 4, () => [0, 0, 0, 255]), picture(4, 5, () => [0, 0, 0, 255]));
  assert.equal(result.ratio, 1);
  assert.equal(result.size, true);
});

test("a file that is not a PNG is refused", () => {
  assert.throws(() => decodePng(Buffer.from("not a picture at all")), /not a PNG/);
});
