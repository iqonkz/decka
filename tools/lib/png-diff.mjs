// Pixel comparison of screenshots without dependencies (T31.1). Reads the PNG files Chrome writes:
// 8 bit, RGB or RGBA, not interlaced.
import { crc32, deflateSync, inflateSync } from "node:zlib";

const SIGNATURE = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);

export function decodePng(buf) {
  if (!buf.subarray(0, 8).equals(SIGNATURE)) throw new Error("not a PNG file");
  let width = 0;
  let height = 0;
  let channels = 0;
  const idat = [];
  for (let at = 8; at + 8 <= buf.length; ) {
    const length = buf.readUInt32BE(at);
    const type = buf.toString("latin1", at + 4, at + 8);
    const data = buf.subarray(at + 8, at + 8 + length);
    if (type === "IHDR") {
      width = data.readUInt32BE(0);
      height = data.readUInt32BE(4);
      const [depth, color, , , interlace] = data.subarray(8, 13);
      if (depth !== 8 || (color !== 2 && color !== 6) || interlace !== 0) throw new Error("unsupported PNG format");
      channels = color === 6 ? 4 : 3;
    } else if (type === "IDAT") idat.push(data);
    else if (type === "IEND") break;
    at += 12 + length;
  }
  const raw = inflateSync(Buffer.concat(idat));
  const stride = width * channels;
  if (!channels || raw.length < (stride + 1) * height) throw new Error("truncated PNG file");
  const rows = Buffer.alloc(stride * height);
  for (let y = 0; y < height; y++) {
    const filter = raw[y * (stride + 1)];
    const line = y * stride;
    for (let x = 0; x < stride; x++) {
      const left = x >= channels ? rows[line + x - channels] : 0;
      const up = y ? rows[line - stride + x] : 0;
      const corner = y && x >= channels ? rows[line - stride + x - channels] : 0;
      let predicted = 0;
      if (filter === 1) predicted = left;
      else if (filter === 2) predicted = up;
      else if (filter === 3) predicted = (left + up) >> 1;
      else if (filter === 4) {
        const p = left + up - corner;
        const pa = Math.abs(p - left);
        const pb = Math.abs(p - up);
        const pc = Math.abs(p - corner);
        predicted = pa <= pb && pa <= pc ? left : pb <= pc ? up : corner;
      }
      rows[line + x] = (raw[y * (stride + 1) + 1 + x] + predicted) & 255;
    }
  }
  const data = Buffer.alloc(width * height * 4, 255);
  for (let i = 0; i < width * height; i++) rows.copy(data, i * 4, i * channels, i * channels + channels);
  return { width, height, data };
}

export function encodePng({ width, height, data }) {
  const chunk = (type, body) => {
    const head = Buffer.concat([Buffer.from(type, "latin1"), body]);
    const out = Buffer.alloc(12 + body.length);
    out.writeUInt32BE(body.length, 0);
    head.copy(out, 4);
    out.writeUInt32BE(crc32(head) >>> 0, 8 + body.length);
    return out;
  };
  const header = Buffer.alloc(13);
  header.writeUInt32BE(width, 0);
  header.writeUInt32BE(height, 4);
  header.set([8, 6, 0, 0, 0], 8);
  const raw = Buffer.alloc((width * 4 + 1) * height);
  for (let y = 0; y < height; y++) data.copy(raw, y * (width * 4 + 1) + 1, y * width * 4, (y + 1) * width * 4);
  return Buffer.concat([SIGNATURE, chunk("IHDR", header), chunk("IDAT", deflateSync(raw)), chunk("IEND", Buffer.alloc(0))]);
}

/**
 * Share of pixels that differ between two decoded pictures. A pixel differs when any colour channel
 * is further apart than `tolerance` (hides anti-aliasing noise). `diff` marks the differing pixels in red
 * over a faded copy of the new picture. Pictures of different size differ completely.
 */
export function comparePixels(a, b, tolerance = 12) {
  if (a.width !== b.width || a.height !== b.height) return { ratio: 1, size: true, diff: null };
  const total = a.width * a.height;
  const diff = Buffer.alloc(total * 4, 255);
  let count = 0;
  for (let i = 0; i < total * 4; i += 4) {
    const changed = Math.abs(a.data[i] - b.data[i]) > tolerance || Math.abs(a.data[i + 1] - b.data[i + 1]) > tolerance || Math.abs(a.data[i + 2] - b.data[i + 2]) > tolerance;
    if (changed) {
      count++;
      diff[i] = 255;
      diff[i + 1] = 0;
      diff[i + 2] = 0;
    } else for (let c = 0; c < 3; c++) diff[i + c] = 192 + (b.data[i + c] >> 2);
  }
  return { ratio: count / total, size: false, diff: { width: a.width, height: a.height, data: diff } };
}
