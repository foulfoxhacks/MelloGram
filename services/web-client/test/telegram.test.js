import assert from "node:assert/strict";
import test from "node:test";
import { tl } from "@mtcute/web";
import { downloadMedia, listChats, relaySocketUrl, resumeSession, sessionExpired } from "../browser/telegram.js";

test("MTProto transport always uses the page origin and validated DC number", () => {
  assert.equal(relaySocketUrl("https://wc.example.com/?external=https://elsewhere.example", 3), "wss://wc.example.com/relay/dc/3");
  assert.equal(relaySocketUrl("http://127.0.0.1:8787/", 2), "ws://127.0.0.1:8787/relay/dc/2");
  for (const id of [0, 6, "2", 1.5, NaN, "https://elsewhere.example"]) assert.throws(() => relaySocketUrl("https://wc.example.com", id));
});

test("only supported user and group chats are listed, with their unread count", async () => {
  const groups = [
    { peer: { id: 1, type: "user", displayName: "Ada", inputPeer: { _: "inputPeerUser", userId: 1, accessHash: 7n } }, unreadCount: 3 },
    { peer: { id: -2, type: "chat", chatType: "group", raw: {}, displayName: "Team", inputPeer: { _: "inputPeerChat", chatId: 2 } }, unreadCount: 0 },
    { peer: { id: -3, type: "chat", chatType: "channel", raw: {}, displayName: "News", inputPeer: {} }, unreadCount: 9 },
    { peer: { id: -4, type: "chat", chatType: "supergroup", raw: { forum: true }, displayName: "Forum", inputPeer: {} }, unreadCount: 1 },
  ];
  const chats = await listChats({ async *iterDialogs() { for (const dialog of groups) yield dialog; } });
  assert.deepEqual(chats.map((chat) => chat.id), [1, -2]);
  assert.deepEqual(chats.map((chat) => chat.unreadCount), [3, 0]);
  assert.equal(chats[0].displayName, "Ada");
});

test("a stored session resumes only when Telegram still accepts the key", async () => {
  const me = { id: 7, displayName: "Ada" };
  assert.equal(await resumeSession({ getMe: async () => me }), me);
  for (const code of ["AUTH_KEY_UNREGISTERED", "SESSION_REVOKED", "USER_DEACTIVATED"]) {
    const error = new tl.RpcError(401, code);
    assert.equal(await resumeSession({ getMe: async () => { throw error; } }), null);
    assert.equal(sessionExpired(error), true);
  }
  const flood = new tl.RpcError(420, "FLOOD_WAIT_30");
  assert.equal(sessionExpired(flood), false);
  await assert.rejects(resumeSession({ getMe: async () => { throw flood; } }), /FLOOD_WAIT_30/);
});

test("media downloads are opaque and reject truncated or oversized bytes", async () => {
  const signal = new AbortController().signal;
  const client = { async *downloadAsIterable() { yield new Uint8Array([1, 2, 3]); } };
  const result = await downloadMedia(client, { fileSize: 3, mimeType: "text/html" }, signal);
  assert.equal(result.type, "application/octet-stream");
  assert.deepEqual([...new Uint8Array(await result.arrayBuffer())], [1, 2, 3]);
  await assert.rejects(downloadMedia(client, { fileSize: 4 }, signal), /Incomplete/);
  const limit = 4 * 1024 * 1024;
  await assert.rejects(downloadMedia(client, { fileSize: limit + 1 }, signal), /limit/);
  const oversize = { async *downloadAsIterable() { yield new Uint8Array(limit); yield new Uint8Array(1); } };
  await assert.rejects(downloadMedia(oversize, {}, signal), /limit/);
});
