// Small pure helpers for the chat console. Kept DOM-free so `node --test` can
// cover them without a browser.

/** One or two initials for an avatar, safe for emoji and empty names. */
export function initials(name) {
  const parts = String(name ?? "").trim().split(/\s+/).filter(Boolean);
  if (!parts.length) return "?";
  const first = [...parts[0]][0] ?? "";
  const last = parts.length > 1 ? [...parts.at(-1)][0] ?? "" : "";
  return (first + last).toUpperCase() || "?";
}

/** Unread badge text, or "" when nothing is unread. */
export function unreadBadge(chat) {
  const count = Math.max(0, Math.trunc(Number(chat?.unreadCount) || 0));
  if (!count) return "";
  return count > 99 ? "99+" : String(count);
}

/** 24-hour clock time for a message, or "" for an invalid date. */
export function messageTime(date) {
  const value = date instanceof Date ? date : new Date(date);
  if (Number.isNaN(value.getTime())) return "";
  return value.toLocaleTimeString("en-GB", { hour: "2-digit", minute: "2-digit" });
}

/** The sender line above a message: "You" for your own, otherwise the name. */
export function senderLine(message) {
  const name = message?.isOutgoing ? "You" : (message?.sender?.displayName || "Unknown");
  const time = messageTime(message?.date);
  return time ? `${name} · ${time}` : name;
}
