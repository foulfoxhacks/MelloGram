// MTProto transport framing for the relay connection.
//
// The relay (src/relay.js) moves opaque bytes between the page and a Telegram
// data centre; this is the browser half. It frames payloads the way MTProto's
// abridged and intermediate transports expect and parses what comes back, and
// nothing else: key exchange, message encryption and the actual API calls are
// separate modules that build on it.
//
// Both transports prefix every payload with its length, in 4-byte units
// (abridged, one byte, or 0x7f plus three bytes when it does not fit) or in
// bytes (intermediate, four-byte little endian). MTProto payloads are always a
// multiple of four bytes, and the intermediate transport pads them to one.

export const ABRIDGED = "abridged";
export const INTERMEDIATE = "intermediate";

/** Refuse anything a data centre would never send, before allocating. */
export const MAX_PACKET = 16 * 1024 * 1024;

/**
 * What a connection writes before its first frame: the transport's tag.
 * Telegram drops a connection whose first byte is a frame header (checked
 * against DC2 — the socket just closes), so the tag is not optional.
 */
const TAGS = Object.freeze({
  [ABRIDGED]: new Uint8Array([0xef]),
  [INTERMEDIATE]: new Uint8Array([0xee, 0xee, 0xee, 0xee]),
});

/** A copy of the tag bytes for `kind`, to send once after the socket opens. */
export function initialBytes(kind = ABRIDGED) {
  const tag = TAGS[kind];
  if (!tag) throw new Error(`unknown transport ${kind}`);
  return tag.slice();
}

const bytesOf = (payload) => {
  if (payload instanceof Uint8Array) return payload;
  if (payload instanceof ArrayBuffer) return new Uint8Array(payload);
  if (ArrayBuffer.isView(payload)) return new Uint8Array(payload.buffer, payload.byteOffset, payload.byteLength);
  throw new TypeError("payload must be bytes");
};

/** Abridged: length / 4 in one byte, or 0x7f + 3 little-endian bytes. */
export function abridgedPacket(payload) {
  const bytes = bytesOf(payload);
  if (!bytes.byteLength || bytes.byteLength % 4 !== 0) {
    throw new Error("an abridged payload is a non-zero multiple of 4 bytes");
  }
  const words = bytes.byteLength / 4;
  if (words < 0x7f) {
    const out = new Uint8Array(1 + bytes.byteLength);
    out[0] = words;
    out.set(bytes, 1);
    return out;
  }
  if (words > 0xffffff) throw new Error("packet is too large for the abridged transport");
  const out = new Uint8Array(4 + bytes.byteLength);
  out[0] = 0x7f;
  out[1] = words & 0xff;
  out[2] = (words >> 8) & 0xff;
  out[3] = (words >> 16) & 0xff;
  out.set(bytes, 4);
  return out;
}

/**
 * Intermediate: four-byte little-endian length that includes the padding, so
 * the frame is a multiple of four bytes. `fill` supplies the padding bytes
 * (random in production, fixed in tests).
 */
export function intermediatePacket(payload, fill = Math.random) {
  const bytes = bytesOf(payload);
  if (!bytes.byteLength) throw new Error("an intermediate payload is not empty");
  const padding = (4 - (bytes.byteLength % 4)) % 4;
  const total = bytes.byteLength + padding;
  if (total + 4 > MAX_PACKET) throw new Error("packet is too large");
  const out = new Uint8Array(4 + total);
  new DataView(out.buffer).setUint32(0, total, true);
  out.set(bytes, 4);
  for (let i = 0; i < padding; i++) out[4 + bytes.byteLength + i] = Math.floor(fill() * 256) & 0xff;
  return out;
}

/**
 * A stateful reader: feed it whatever the socket delivered and it returns the
 * payloads that are now complete. A frame split across several chunks is held
 * until its last byte arrives.
 */
export function frameReader(kind = ABRIDGED) {
  if (kind !== ABRIDGED && kind !== INTERMEDIATE) throw new Error(`unknown transport ${kind}`);
  let buffer = new Uint8Array(0);
  return (chunk) => {
    const incoming = bytesOf(chunk);
    if (incoming.byteLength) {
      const merged = new Uint8Array(buffer.byteLength + incoming.byteLength);
      merged.set(buffer, 0);
      merged.set(incoming, buffer.byteLength);
      buffer = merged;
    }
    const frames = [];
    for (;;) {
      let header;
      let length;
      if (kind === ABRIDGED) {
        if (!buffer.byteLength) break;
        if (buffer[0] === 0x7f) {
          if (buffer.byteLength < 4) break;
          length = ((buffer[1] | (buffer[2] << 8) | (buffer[3] << 16)) >>> 0) * 4;
          header = 4;
        } else {
          length = buffer[0] * 4;
          header = 1;
        }
      } else {
        if (buffer.byteLength < 4) break;
        length = (buffer[0] | (buffer[1] << 8) | (buffer[2] << 16) | (buffer[3] << 24)) >>> 0;
        header = 4;
      }
      if (!length) throw new Error("a zero-length frame is never valid");
      if (length > MAX_PACKET) throw new Error(`frame of ${length} bytes exceeds the limit`);
      if (buffer.byteLength < header + length) break;
      frames.push(buffer.slice(header, header + length));
      buffer = buffer.slice(header + length);
    }
    return frames;
  };
}

/** Frames a payload for the transport named by `kind`. */
export function frame(kind, payload) {
  return kind === ABRIDGED ? abridgedPacket(payload) : intermediatePacket(payload);
}
