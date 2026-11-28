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
        handle ? h("span", { class: "mg-muted", text: formatPresence(p) ? ` · ${handle}` : handle }) : null);
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
  // A titled group of blocks. Returns null when it would be empty, so callers
  // can list it with the rest and filter once.
  const section = (title, ...children) => {
    const kids = children.flat().filter((c) => c != null && c !== false);
    return kids.length ? h("section", { class: "mg-info-section" }, h("h4", { text: title }), ...kids) : null;
  };
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
    if (!info) { panel.replaceChildren(closeBtn(), h("p", { class: "mg-hint", text: "No details are available for this chat." })); return; }
    const kindLabel = { user: info.bot ? "Bot" : "User", group: "Group", supergroup: "Supergroup", channel: "Channel" }[info.kind] || info.kind;
    panelPresence.textContent = info.kind === "user" ? formatPresence(presences.get(Number(chat.id)) || info.presence) : "";
    const src = document.querySelector(`.mg-chat-item[data-chat-id="${chat.id}"] .mg-avatar`);
    // Reuse the list avatar (with its photo, if loaded); otherwise initials.
    const avatar = src ? src.cloneNode(true) : h("div", { class: "mg-avatar", text: (info.title || chat.title || "?").split(/\s+/).map((w) => [...w][0] || "").join("").slice(0, 2).toUpperCase() });
    avatar.classList.add("mg-info-avatar");
    avatar.classList.remove("mg-avatar-online");
    // Full-size (640px) photo for the panel; the list keeps the small one.
    if (info.photo) {
      Bridge.invoke("chat_photo", { chatId: chat.id, photoId: info.photo, big: true }).then((url) => {
        if (!url || !avatar.isConnected) return;
        const img = h("img", { class: "mg-avatar-img", alt: "", src: url, draggable: false });
        img.addEventListener("load", () => {
          // Drop every earlier image (the list's small one, or a previous
          // panel photo), not just the first, so two photos can never overlap.
          for (const old of avatar.querySelectorAll(".mg-avatar-img")) if (old !== img) old.remove();
          avatar.classList.add("mg-has-photo");
        });
        // A big photo that fails to decode must not leave a hole: the list's
        // small photo (or the initials) stays underneath.
        img.addEventListener("error", () => img.remove());
        avatar.append(img);
        avatar.classList.add("mg-clickable");
        avatar.title = "Open photo";
        avatar.onclick = () => window.MelloGram.Media?.viewer(url, "img");
      }).catch(() => {});
    }
    const link = info.username ? `https://t.me/${info.username}` : info.invite_link;

    const members = h("div", { class: "mg-info-members" });
    panel.replaceChildren(...[
      closeBtn(),
      h("div", { class: "mg-info-head" }, avatar,
        h("h3", { text: info.title || chat.title }),
        h("div", { class: "mg-info-badges" },
          h("span", { class: "mg-tag", text: kindLabel }),
          info.verified ? h("span", { class: "mg-tag mg-tag-good", text: "Verified" }) : null,
          info.premium ? h("span", { class: "mg-tag mg-tag-premium", text: "★ Premium" }) : null,
          info.mutual_contact ? h("span", { class: "mg-tag", text: "Mutual contact" }) : info.contact ? h("span", { class: "mg-tag", text: "Contact" }) : null,
          info.business_open_now === true ? h("span", { class: "mg-tag mg-tag-good", text: "Open now" }) : info.business_open_now === false ? h("span", { class: "mg-tag", text: "Closed now" }) : null,
          info.scam ? h("span", { class: "mg-tag mg-tag-bad", text: "Flagged as scam/fake" }) : null,
          info.blocked ? h("span", { class: "mg-tag mg-tag-bad", text: "Blocked" }) : null),
        panelPresence),
      section("About",
        info.about ? h("p", { class: "mg-info-about", text: info.about }) : null,
        info.bot_description ? h("p", { class: "mg-info-about", text: info.bot_description }) : null,
        info.business_intro ? h("p", { class: "mg-info-about", text: info.business_intro }) : null),
      section("Details", h("div", { class: "mg-info-rows" },
        row("Username", info.username ? `@${info.username}` : null),
        row("Phone", info.phone ? `+${info.phone}` : null),
        row("Also known as", info.extra_usernames?.length ? info.extra_usernames.map((u) => `@${u}`).join(", ") : null),
        row("Birthday", info.birthday),
        row("Business address", info.business_location),
        row("Business hours", info.business_timezone ? `Time zone ${info.business_timezone}` : null),
        row("Location", info.location),
        row("Language", info.language),
        row("Gifts", info.gifts?.toLocaleString()),
        row("Boosts", info.boosts?.toLocaleString()),
        row("Join requests", info.join_requests?.toLocaleString()),
        row(info.kind === "channel" ? "Subscribers" : "Members", info.members?.toLocaleString()),
        row("Online now", info.online?.toLocaleString()),
        row("Admins", info.admins),
        row("Banned", info.kicked),
        row("Restricted", info.banned),
        row("Groups in common", info.common_chats),
        row("Slow mode", info.slow_mode_seconds ? dur(info.slow_mode_seconds) : null),
        row("Auto-delete", info.auto_delete_seconds ? dur(info.auto_delete_seconds) : null),
        row("ID", chat.id))),
      h("div", { class: "mg-toolbar" },
        link ? h("button", { class: "mg-btn", text: "Copy link", onclick: () => navigator.clipboard.writeText(link).then(() => UI.toast("Link copied", "success")) }) : null,
        h("button", { class: "mg-btn", text: "Copy ID", onclick: () => navigator.clipboard.writeText(String(chat.id)).then(() => UI.toast("ID copied", "success")) }),
        info.personal_channel_id ? h("button", { class: "mg-btn", text: "Personal channel", onclick: () => App().openChatById(info.personal_channel_id).catch((e) => UI.toast(String(e), "info")) }) : null,
        info.pinned_message_id ? h("button", { class: "mg-btn", text: "Pinned message", onclick: () => App().jumpTo(info.pinned_message_id) }) : null,
        info.linked_chat_id ? h("button", { class: "mg-btn", text: info.kind === "channel" ? "Discussion group" : "Linked channel", onclick: () => App().openChatById(info.linked_chat_id).catch((e) => UI.toast(String(e), "info")) }) : null),
      members,
    ].filter(Boolean));

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
