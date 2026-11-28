import assert from "node:assert/strict";
import test from "node:test";

import { initials, messageTime, senderLine, unreadBadge } from "../public/chat-format.js";

test("initials survive single names, emoji and empty values", () => {
  assert.equal(initials("Ada Lovelace"), "AL");
  assert.equal(initials("MelloGram"), "M");
  assert.equal(initials("  ada   byron  lovelace "), "AL");
  assert.equal(initials("🦊 Fox"), "🦊F");
  assert.equal(initials(""), "?");
  assert.equal(initials(undefined), "?");
});

test("unread badges cap at 99+ and stay empty when read", () => {
  assert.equal(unreadBadge({ unreadCount: 0 }), "");
  assert.equal(unreadBadge({}), "");
  assert.equal(unreadBadge({ unreadCount: -3 }), "");
  assert.equal(unreadBadge({ unreadCount: 7 }), "7");
  assert.equal(unreadBadge({ unreadCount: 99 }), "99");
  assert.equal(unreadBadge({ unreadCount: 1400 }), "99+");
  assert.equal(unreadBadge({ unreadCount: 2.9 }), "2");
});

test("message times are 24-hour and invalid dates stay blank", () => {
  assert.match(messageTime(new Date(2026, 10, 28, 9, 5)), /^09:05$/);
  assert.match(messageTime("2026-11-28T21:30:00Z"), /^\d{2}:\d{2}$/);
  assert.equal(messageTime("not a date"), "");
  assert.equal(messageTime(undefined), "");
});

test("the sender line says You for outgoing messages", () => {
  const date = new Date(2026, 10, 28, 14, 4);
  assert.match(senderLine({ isOutgoing: true, date }), /^You · 14:04$/);
  assert.match(senderLine({ isOutgoing: false, date, sender: { displayName: "Ada" } }), /^Ada · 14:04$/);
  assert.match(senderLine({ isOutgoing: false, sender: {} }), /^Unknown$/);
});
