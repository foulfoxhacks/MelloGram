import test from "node:test";
import assert from "node:assert/strict";

import worker from "../src/worker.js";
import {
  DC_ADDRESSES,
  DC_PORT,
  MAX_BYTES,
  MAX_CONNECTIONS,
  MAX_SECONDS,
  bytesOfMessage,
  relayRejection,
  relayTarget,
  relayUrl,
} from "../src/relay.js";

const env = () => ({
  PUBLIC_BASE_URL: "https://wc.example.com",
  API_BASE_URL: "https://wc-api.example.com",
  ASSETS: { fetch: async () => new Response("<!" + "doctype html>") },
});

const relayCall = (dc, headers = {}) =>
  worker.fetch(new Request(`https://wc-api.example.com/relay/dc/${dc}`, { headers }), env());

test("the relay only reaches Telegram's five production data centres", () => {
  assert.deepEqual(Object.keys(DC_ADDRESSES), ["1", "2", "3", "4", "5"]);
  for (const dc of Object.keys(DC_ADDRESSES)) {
    const target = relayTarget(dc);
    assert.equal(target.port, DC_PORT);
    assert.match(target.hostname, /^\d{1,3}(\.\d{1,3}){3}$/, dc);
  }
  // Nothing that is not a data-centre number, and nothing that could be a host.
  for (const bad of ["", "0", "6", "12", "2x", "../2", "2%00", "-2", "abc", null, undefined]) {
    assert.equal(relayTarget(bad), null, String(bad));
  }
});

test("the relay refuses anything but a same-origin WebSocket upgrade", () => {
  const allowed = "https://wc.example.com";
  assert.equal(relayRejection(new Request("https://wc-api.example.com/relay/dc/2"), { PUBLIC_BASE_URL: allowed }), "upgrade-required");
  assert.equal(
    relayRejection(new Request("https://wc-api.example.com/relay/dc/2", { headers: { Upgrade: "websocket" } }), { PUBLIC_BASE_URL: allowed }),
    "origin",
  );
  assert.equal(
    relayRejection(
      new Request("https://wc-api.example.com/relay/dc/2", { headers: { Upgrade: "websocket", Origin: "https://evil.example" } }),
      { PUBLIC_BASE_URL: allowed },
    ),
    "origin",
  );
  assert.equal(
    relayRejection(
      new Request("https://wc-api.example.com/relay/dc/2", { headers: { Upgrade: "websocket", Origin: allowed } }),
      { PUBLIC_BASE_URL: allowed },
    ),
    null,
  );
  // Without a configured origin nothing may connect at all.
  assert.equal(
    relayRejection(new Request("https://wc-api.example.com/relay/dc/2", { headers: { Upgrade: "websocket", Origin: allowed } }), {}),
    "origin",
  );
});

test("the relay route answers before any socket exists", async () => {
  const notUpgrade = await relayCall(2);
  assert.equal(notUpgrade.status, 426);
  assert.equal(notUpgrade.headers.get("upgrade"), "websocket");
  assert.equal(notUpgrade.headers.get("cache-control"), "no-store");

  assert.equal((await relayCall(9)).status, 404);
  const foreign = await relayCall(2, { Upgrade: "websocket", Origin: "https://evil.example" });
  assert.equal(foreign.status, 403);
});

test("relay limits are real numbers, not placeholders", () => {
  assert.ok(MAX_BYTES >= 1024 * 1024);
  assert.ok(MAX_SECONDS >= 60);
  assert.ok(MAX_CONNECTIONS >= 1 && MAX_CONNECTIONS <= 8);
});

test("the page is told to connect to the API host over wss", () => {
  assert.equal(relayUrl("https://wc-api.example.com", 2), "wss://wc-api.example.com/relay/dc/2");
  assert.equal(relayUrl("http://127.0.0.1:8787", 4), "ws://127.0.0.1:8787/relay/dc/4");
});

test("a socket message becomes bytes whatever shape workerd delivers", () => {
  const payload = new Uint8Array([0xef, 0x0a, 0x01]);
  assert.deepEqual([...bytesOfMessage(payload.buffer)], [...payload]);
  assert.deepEqual([...bytesOfMessage(payload)], [...payload]);

  // A view with an offset must not drag in the rest of its buffer.
  const wide = new Uint8Array([9, 9, 1, 2, 3, 9]);
  assert.deepEqual([...bytesOfMessage(new DataView(wide.buffer, 2, 3))], [1, 2, 3]);

  // Text is not MTProto, and a wrong `ArrayBuffer` assumption used to turn
  // every message into zero bytes, so the relay forwarded nothing.
  assert.equal(bytesOfMessage("hello"), null);
  assert.equal(bytesOfMessage(undefined), null);
});

test("the page's policy allows the relay socket and nothing else", async () => {
  const page = await worker.fetch(new Request("https://wc.example.com/"), env());
  const csp = page.headers.get("content-security-policy");
  assert.match(csp, /connect-src 'self' https:\/\/wc-api\.example\.com wss:\/\/wc-api\.example\.com/);
});
