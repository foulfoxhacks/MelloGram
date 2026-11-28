import test from "node:test";
import assert from "node:assert/strict";

import {
  ABRIDGED,
  INTERMEDIATE,
  MAX_PACKET,
  abridgedPacket,
  frameReader,
  initialBytes,
  intermediatePacket,
} from "../public/mtproto/transport.js";

const bytes = (...values) => new Uint8Array(values);
const payload = (length, seed = 7) => Uint8Array.from({ length }, (_, i) => (i * seed) & 0xff);

test("abridged framing: one byte while the length fits, 0x7f beyond it", () => {
  const small = abridgedPacket(payload(8));
  assert.equal(small.byteLength, 9);
  assert.equal(small[0], 2); // 8 bytes = 2 four-byte words
  assert.deepEqual([...small.slice(1)], [...payload(8)]);

  const big = abridgedPacket(payload(127 * 4));
  assert.equal(big[0], 0x7f);
  assert.equal(big[1] | (big[2] << 8) | (big[3] << 16), 127);
  assert.equal(big.byteLength, 4 + 127 * 4);

  // The last length that still fits in the short form.
  assert.equal(abridgedPacket(payload(126 * 4))[0], 126);
});

test("abridged framing refuses what MTProto cannot carry", () => {
  assert.throws(() => abridgedPacket(new Uint8Array(0)), /non-zero multiple of 4/);
  assert.throws(() => abridgedPacket(new Uint8Array(7)), /non-zero multiple of 4/);
  assert.throws(() => abridgedPacket("not bytes"), TypeError);
});

test("intermediate framing pads to four bytes and counts the padding", () => {
  // `fill` returns a fraction in [0, 1); 0.5 pads with 128, which is what the
  // random padding looks like in production.
  const five = intermediatePacket(payload(5), () => 0.5);
  assert.equal(five.byteLength, 4 + 8);
  assert.equal(new DataView(five.buffer).getUint32(0, true), 8);
  assert.deepEqual([...five.slice(4, 9)], [...payload(5)]);
  assert.deepEqual([...five.slice(9)], [128, 128, 128]);

  const eight = intermediatePacket(payload(8), () => 0xcd);
  assert.equal(eight.byteLength, 12);
  assert.equal(new DataView(eight.buffer).getUint32(0, true), 8);
});

test("the reader reassembles frames split across chunks", () => {
  const read = frameReader(ABRIDGED);
  const packet = abridgedPacket(payload(12));
  assert.deepEqual(read(packet.slice(0, 3)), []);
  assert.deepEqual(read(packet.slice(3, 7)), []);
  const frames = read(packet.slice(7));
  assert.equal(frames.length, 1);
  assert.deepEqual([...frames[0]], [...payload(12)]);
});

test("the reader handles several frames in one chunk and both transports", () => {
  for (const [kind, frame] of [
    [ABRIDGED, (p) => abridgedPacket(p)],
    [INTERMEDIATE, (p) => intermediatePacket(p, () => 0)],
  ]) {
    const read = frameReader(kind);
    const a = payload(4, 3);
    const b = payload(16, 5);
    const merged = new Uint8Array([...frame(a), ...frame(b)]);
    const frames = read(merged);
    assert.equal(frames.length, 2, kind);
    assert.deepEqual([...frames[0]], [...a], kind);
    assert.deepEqual([...frames[1]], [...b], kind);
  }
});

test("the reader rejects impossible lengths instead of allocating", () => {
  const read = frameReader(INTERMEDIATE);
  const bogus = new Uint8Array(8);
  new DataView(bogus.buffer).setUint32(0, MAX_PACKET + 4, true);
  assert.throws(() => read(bogus), /exceeds the limit/);

  const zero = frameReader(ABRIDGED);
  assert.throws(() => zero(bytes(0)), /zero-length/);

  assert.throws(() => frameReader("turbo"), /unknown transport/);
});

test("a connection opens with its transport tag, which Telegram requires", () => {
  assert.deepEqual([...initialBytes(ABRIDGED)], [0xef]);
  assert.deepEqual([...initialBytes(INTERMEDIATE)], [0xee, 0xee, 0xee, 0xee]);
  assert.deepEqual([...initialBytes()], [0xef]);
  assert.throws(() => initialBytes("turbo"), /unknown transport/);

  // The caller owns what it sends; a second connection must not share the tag.
  const first = initialBytes(ABRIDGED);
  first[0] = 0;
  assert.equal(initialBytes(ABRIDGED)[0], 0xef);
});
