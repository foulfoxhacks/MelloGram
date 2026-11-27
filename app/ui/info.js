// Chat / group / channel info panel and precise presence ("last seen today at 14:03:21").
"use strict";

(() => {
  const { Bridge, UI, Hooks, h } = window.MelloGram;
  const $ = (id) => document.getElementById(id);
  const App = () => window.MelloGram.App;

  const presences = new Map(); // user id -> presence
  const pad = (n) => String(n).padStart(2, "0");

  function exactTime(seconds) {
    const d = new Date(seconds * 1000);
    return `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
  }

  /** Human, precise presence text. */
  function formatPresence(p) {
    if (!p) return "";
    switch (p.state) {
      case "online": return "online";
      case "last-seen": {
        const d = new Date(p.at * 1000);
        const now = new Date();
        const yesterday = new Date(now); yesterday.setDate(now.getDate() - 1);
        const mins = Math.round((Date.now() - d) / 60000);
        if (mins < 1) return "last seen just now";
        if (mins < 60) return `last seen ${mins} minute${mins === 1 ? "" : "s"} ago (${exactTime(p.at)})`;
        if (d.toDateString() === now.toDateString()) return `last seen today at ${exactTime(p.at)}`;
        if (d.toDateString() === yesterday.toDateString()) return `last seen yesterday at ${exactTime(p.at)}`;
        return `last seen ${d.toLocaleDateString([], { day: "numeric", month: "short", year: "numeric" })} at ${exactTime(p.at)}`;
      }
      case "recently": return "last seen recently (hidden)";
      case "within-week": return "last seen within a week (hidden)";
      case "within-month": return "last seen within a month (hidden)";
      case "bot": return "bot";
      default: return "last seen a long time ago";
    }
  }
  const isOnline = (p) => p?.state === "online" && p.until * 1000 > Date.now();

  // userId for a one-to-one chat id (Bot-API ids: users are positive).
  const userIdOf = (chat) => (chat && chat.kind === "user" ? Number(chat.id) : null);

  function presenceFor(chat) {
    const uid = userIdOf(chat);
    return uid != null ? presences.get(uid) || chat.presence : null;
  }

  async function updateHeader(chat) {
    if (!chat || App().currentChat()?.id !== chat.id) return;
    const sub = $("chat-sub");
    const handle = chat.username ? `@${chat.username}` : "";
    if (chat.kind === "user") {
      const p = presenceFor(chat);
      sub.replaceChildren(
        h("span", { class: `mg-presence${isOnline(p) ? " mg-online" : ""}`, title: p?.state === "last-seen" ? new Date(p.at * 1000).toString() : "", text: formatPresence(p) }),
        handle ? h("span", { class: "mg-muted", text: ` · ${handle}` }) : null);
      return;
    }
    sub.textContent = [handle, chat.kind].filter(Boolean).join(" · ");
    try {
      const info = await Bridge.invoke("chat_info", { chatId: chat.id });
      if (App().currentChat()?.id !== chat.id) return;
      const parts = [];
      if (info.members != null) parts.push(`${info.members.toLocaleString()} ${info.kind === "channel" ? "subscribers" : "members"}`);
      if (info.online) parts.push(`${info.online.toLocaleString()} online`);
      if (handle) parts.push(handle);
      sub.textContent = parts.join(" · ") || chat.kind;
    } catch { /* offline or no rights: keep the basic line */ }
  }

  Hooks.chatOpen.add((chat) => { updateHeader(chat); if (!panel.hidden) showInfo(chat); });

  Bridge.listen("mg://event", (ev) => {
    if (ev.type !== "presence") return;
    presences.set(Number(ev.user_id), ev.presence);
    const chat = App().currentChat();
    if (chat && userIdOf(chat) === Number(ev.user_id)) updateHeader(chat);
    if (!panel.hidden && chat && userIdOf(chat) === Number(ev.user_id)) panelPresence.textContent = formatPresence(ev.presence);
  });
  // Re-render relative times once a minute.
  setInterval(() => updateHeader(App().currentChat()), 60000);

  // Online dot in the chat list.
  Hooks.chatRender.add((chat, el) => {
    if (isOnline(presenceFor(chat))) el.querySelector(".mg-avatar")?.classList.add("mg-avatar-online");
  });

  // ---- info panel -----------------------------------------------------------------

  const panel = h("aside", { class: "mg-info-panel", hidden: true, "aria-label": "Chat information" });
  const panelPresence = h("div", { class: "mg-info-presence" });
  document.getElementById("app").append(panel);
  $("chat-title").parentElement.classList.add("mg-clickable");
  $("chat-title").parentElement.title = "Show chat info";
  $("chat-title").parentElement.addEventListener("click", () => {
    const chat = App().currentChat();
    if (!chat) return;
    if (panel.hidden) showInfo(chat); else panel.hidden = true;
  });

  const row = (label, value) => (value == null || value === "" ? null : h("div", { class: "mg-info-row" }, h("span", { class: "mg-muted", text: label }), h("span", { text: String(value) })));
  const plural = (n, unit) => `${n} ${unit}${n === 1 ? "" : "s"}`;
  const dur = (s) => (s >= 86400 ? plural(Math.round(s / 86400), "day") : s >= 3600 ? plural(Math.round(s / 3600), "hour") : s >= 60 ? plural(Math.round(s / 60), "minute") : plural(s, "second"));

  async function showInfo(chat) {
    panel.hidden = false;
    panel.replaceChildren(h("p", { class: "mg-hint", text: "Loading…" }));
    let info;
    try {
      info = await Bridge.invoke("chat_info", { chatId: chat.id });
    } catch (e) {
      panel.replaceChildren(closeBtn(), h("p", { class: "mg-error", text: String(e) }));
      return;
    }
    if (App().currentChat()?.id !== chat.id) return;
    const kindLabel = { user: info.bot ? "Bot" : "User", group: "Group", supergroup: "Supergroup", channel: "Channel" }[info.kind] || info.kind;
    panelPresence.textContent = info.kind === "user" ? formatPresence(presences.get(Number(chat.id)) || info.presence) : "";
    const avatar = h("div", { class: "mg-avatar mg-info-avatar", text: (info.title || chat.title || "?").split(/\s+/).map((w) => [...w][0] || "").join("").slice(0, 2).toUpperCase() });
    const src = document.querySelector(`.mg-chat-item[data-chat-id="${chat.id}"] .mg-avatar`);
    if (src) avatar.style.background = src.style.background;
    const link = info.username ? `https://t.me/${info.username}` : info.invite_link;

    const members = h("div", { class: "mg-info-members" });
    panel.replaceChildren(
      closeBtn(),
      h("div", { class: "mg-info-head" }, avatar,
        h("h3", { text: info.title || chat.title }),
        h("div", { class: "mg-info-badges" },
          h("span", { class: "mg-tag", text: kindLabel }),
          info.verified ? h("span", { class: "mg-tag mg-tag-good", text: "Verified" }) : null,
          info.scam ? h("span", { class: "mg-tag mg-tag-bad", text: "Flagged as scam/fake" }) : null,
          info.blocked ? h("span", { class: "mg-tag mg-tag-bad", text: "Blocked" }) : null),
        panelPresence),
      info.about ? h("p", { class: "mg-info-about", text: info.about }) : null,
      h("div", { class: "mg-info-rows" },
        row("Username", info.username ? `@${info.username}` : null),
        row("Phone", info.phone ? `+${info.phone}` : null),
        row("Birthday", info.birthday),
        row(info.kind === "channel" ? "Subscribers" : "Members", info.members?.toLocaleString()),
        row("Online now", info.online?.toLocaleString()),
        row("Admins", info.admins),
        row("Banned", info.kicked),
        row("Restricted", info.banned),
        row("Groups in common", info.common_chats),
        row("Slow mode", info.slow_mode_seconds ? dur(info.slow_mode_seconds) : null),
        row("Auto-delete", info.auto_delete_seconds ? dur(info.auto_delete_seconds) : null),
        row("ID", chat.id)),
      h("div", { class: "mg-toolbar" },
        link ? h("button", { class: "mg-btn", text: "Copy link", onclick: () => navigator.clipboard.writeText(link).then(() => UI.toast("Link copied", "success")) }) : null,
        h("button", { class: "mg-btn", text: "Copy ID", onclick: () => navigator.clipboard.writeText(String(chat.id)).then(() => UI.toast("ID copied", "success")) }),
        info.pinned_message_id ? h("button", { class: "mg-btn", text: "Pinned message", onclick: () => App().jumpTo(info.pinned_message_id) }) : null,
        info.linked_chat_id ? h("button", { class: "mg-btn", text: info.kind === "channel" ? "Discussion group" : "Linked channel", onclick: () => App().openChatById(info.linked_chat_id).catch((e) => UI.toast(String(e), "info")) }) : null),
      members);

    if (info.kind !== "user") {
      if (info.members_hidden && !info.can_view_members) {
        members.append(h("p", { class: "mg-hint", text: "The member list is hidden by the group's admins." }));
      } else {
        members.append(h("button", { class: "mg-btn", text: "Show members", onclick: (e) => loadMembers(chat, members, e.target) }));
      }
    }
  }

  async function loadMembers(chat, box, btn) {
    btn.disabled = true;
    btn.textContent = "Loading members…";
    try {
      const list = await Bridge.invoke("chat_members", { chatId: chat.id, limit: 200 });
      const order = { owner: 0, admin: 1, member: 2, banned: 3, left: 4 };
      list.sort((a, b) => (order[a.role] - order[b.role]) || (isOnline(b.presence) - isOnline(a.presence)) || a.name.localeCompare(b.name));
      box.replaceChildren(h("h4", { text: `Members (${list.length}${list.length >= 200 ? "+" : ""})` }),
        h("div", { class: "mg-list" }, ...list.map((m) => h("div", { class: "mg-list-item mg-member" },
          h("div", { class: "mg-list-main" },
            h("b", { text: m.name || "Deleted account" }),
            h("div", { class: "mg-hint", text: [m.username && `@${m.username}`, formatPresence(m.presence)].filter(Boolean).join(" · ") })),
          m.role !== "member" ? h("span", { class: `mg-tag mg-role-${m.role}`, text: m.role }) : null))));
    } catch (e) {
      box.replaceChildren(h("p", { class: "mg-hint", text: `Members are not available: ${e}` }));
    }
  }

  function closeBtn() {
    return h("button", { class: "mg-icon-btn mg-info-close", title: "Close", text: "✕", onclick: () => (panel.hidden = true) });
  }

  window.MelloGram.Presence = { format: formatPresence, isOnline };
})();
