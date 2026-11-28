import assert from "node:assert/strict";
import test from "node:test";
import { downloadMedia, listChats, relaySocketUrl } from "../browser/telegram.js";

test("MTProto transport always uses the page origin and validated DC number", () => {
  assert.equal(relaySocketUrl("https://wc.example.com/?external=https://elsewhere.example", 3), "wss://wc.example.com/relay/dc/3");
  assert.equal(relaySocketUrl("http://127.0.0.1:8787/", 2), "ws://127.0.0.1:8787/relay/dc/2");
  for (const id of [0, 6, "2", 1.5, NaN, "https://elsewhere.example"]) assert.throws(() => relaySocketUrl("https://wc.example.com", id));
});

test("only supported user and group chats are listed", async () => {
  const groups = [
    { id: 1, type: "user" },
    { id: -2, type: "chat", chatType: "group", raw: {} },
    { id: -3, type: "chat", chatType: "channel", raw: {} },
    { id: -4, type: "chat", chatType: "supergroup", raw: { forum: true } },
  ];
  const chats = await listChats({ async *iterDialogs() { for (const peer of groups) yield { peer }; } });
  assert.deepEqual(chats, groups.slice(0, 2));
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
