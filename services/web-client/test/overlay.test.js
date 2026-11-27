// The committed wasm-bindgen output is tested in Node, so the browser bundle
// cannot drift from the protocol crate without CI noticing.
import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

import { initSync, WasmIdentity, WasmSession, accept } from "../public/wasm/mellogram_overlay.js";

const bytes = await readFile(new URL("../public/wasm/mellogram_overlay_bg.wasm", import.meta.url));
await initSync({ module: bytes });

test("identities agree on a 60-digit safety number", () => {
  const alice = WasmIdentity.generate();
  const bob = WasmIdentity.generate();
  const fromAlice = alice.safetyNumber(bob.bundle());
  const fromBob = bob.safetyNumber(alice.bundle());
  assert.equal(fromAlice, fromBob);
  assert.equal(fromAlice.replace(/ /g, "").length, 60);
  assert.match(fromAlice, /^\d{5}( \d{5}){11}$/);
  assert.notEqual(alice.bundle(), bob.bundle());
});

test("a pairwise conversation round-trips through the bindings", () => {
  const alice = WasmIdentity.generate();
  const bob = WasmIdentity.generate();
  const chat = 424242;

  const out = WasmSession.start(alice, bob.bundle(), chat);
  assert.equal(out.awaitingReply(), true);
  const hello = out.encrypt("hello from wasm", chat);
  assert.match(hello, /^mg-e2e:v1:/);

  const accepted = JSON.parse(accept(bob, hello, chat));
  assert.equal(accepted.plaintext, "hello from wasm");
  const inbox = WasmSession.fromState(Buffer.from(accepted.state, "base64"));

  out.decrypt(inbox.encrypt("hi back", chat), chat);
  assert.equal(out.awaitingReply(), false);
  const reply = inbox.decrypt(out.encrypt("third", chat), chat);
  assert.equal(reply, "third");
  assert.equal(out.safetyNumber(), inbox.safetyNumber());
});

test("session state restores", () => {
  const alice = WasmIdentity.generate();
  const bob = WasmIdentity.generate();
  const chat = 99;
  const session = WasmSession.start(alice, bob.bundle(), chat);
  const saved = session.state();
  accept(bob, session.encrypt("first", chat), chat);
  const restored = WasmSession.fromState(saved);
  assert.match(restored.encrypt("second", chat), /^mg-e2e:v1:/);
});

test("the committed bindings expose the identity secret for the caller to seal", () => {
  const alice = WasmIdentity.generate();
  const restored = WasmIdentity.fromSecret(alice.secretBytes());
  assert.equal(restored.bundle(), alice.bundle());
});
