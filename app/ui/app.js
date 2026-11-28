// MelloGram main window: login, chat list, messages and composer.
"use strict";

(() => {
  const { Bridge, UI, Hooks, PluginHost, h } = window.MelloGram;
  const $ = (id) => document.getElementById(id);

  const state = {
    chats: [],
    current: null,          // chat summary
    messages: new Map(),    // chatId -> Map(msgId -> message)
    replyTo: null,
    attachments: [],
    commands: [],
    commandIndex: 0,
    lastTyping: 0,
  };

  const App = {
    renderChats,
    refreshChats,
    rerenderMessages: () => renderMessages(false),
    currentChat: () => state.current,
    loadedMessages: () => (state.current ? [...(state.messages.get(state.current.id)?.values() || [])].sort((a, b) => a.id - b.id) : []),
    jumpTo: (id) => jumpTo(id),
    async openChatById(id) {
      const chat = state.chats.find((c) => String(c.id) === String(id));
      if (!chat) throw new Error("That chat is not in your chat list");
      if (state.current?.id !== chat.id) await openChat(chat);
    },
    chats: () => state.chats,
    addMessage: (m) => upsert(m),
    openChat: (c) => openChat(c),
  };
  window.MelloGram.App = App;

  // ---- styles -------------------------------------------------------------

  async function applyStylesheet(css) {
    $("mg-user-css").textContent = css ?? (await Bridge.invoke("get_stylesheet"));
  }

  // ---- login ----------------------------------------------------------------

  const loginSteps = {
    credentials: {
      help: "MelloGram talks to Telegram with your own API credentials. Create them at my.telegram.org → API development tools.",
      fields: () => [
        field("API ID", h("input", { type: "number", name: "api_id", required: true, autocomplete: "off" })),
        field("API hash", h("input", { type: "password", name: "api_hash", required: true, autocomplete: "off", spellcheck: false })),
      ],
      submit: async (f) => {
        await Bridge.invoke("set_credentials", { apiId: Number(f.api_id.value), apiHash: f.api_hash.value });
        return Bridge.invoke("login_state");
      },
    },
    "need-phone": {
      help: "Enter your phone number in international format.",
      fields: () => [field("Phone number", h("input", { type: "tel", name: "phone", placeholder: "+1 555 123 4567", required: true }))],
      submit: (f) => Bridge.invoke("request_code", { phone: f.phone.value }),
    },
    "need-code": {
      help: (s) => `We sent a code to your Telegram app${s.phone ? ` for ${s.phone}` : ""}.`,
      fields: () => [field("Login code", h("input", { type: "text", name: "code", inputmode: "numeric", required: true, autocomplete: "one-time-code" }))],
      submit: (f) => Bridge.invoke("submit_code", { code: f.code.value }),
    },
    "need-password": {
      help: (s) => `This account has two-step verification.${s.hint ? ` Hint: ${s.hint}` : ""}`,
      fields: () => [field("Password", h("input", { type: "password", name: "password", required: true }))],
      submit: (f) => Bridge.invoke("submit_password", { password: f.password.value }),
    },
  };

  function field(label, input) {
    return h("label", {}, label, input);
  }

  let loginStep = null;
  function showLogin(s) {
    const key = s ? s.state : "credentials";
    if (key === "ready") { $("login").hidden = true; onReady(s.me); return; }
    loginStep = { key, s: s || {} };
    const step = loginSteps[key];
    $("login-help").textContent = typeof step.help === "function" ? step.help(s) : step.help;
    $("login-fields").replaceChildren(...step.fields());
    $("login-error").hidden = true;
    $("login").hidden = false;
    $("login-fields").querySelector("input")?.focus();
  }

  $("login-form").addEventListener("submit", async (e) => {
    e.preventDefault();
    const btn = $("login-submit");
    btn.disabled = true;
    try {
      showLogin(await loginSteps[loginStep.key].submit(e.target.elements));
    } catch (err) {
      $("login-error").textContent = String(err);
      $("login-error").hidden = false;
    } finally {
      btn.disabled = false;
    }
  });

  // Layout style (Settings → General), applied once per launch.
  Bridge.invoke("get_settings").then((s) => {
    const layout = s?.general?.layout;
    if (["classic", "compact", "wide"].includes(layout)) document.body.classList.add(`mg-layout-${layout}`);
  }).catch(() => {});

  async function onReady(me) {
    document.title = `MelloGram — ${me.name}`;
    // "Check for updates on launch" (Settings → General): one request to
    // GitHub's public releases API, nothing about this install is sent.
    Bridge.invoke("get_settings").then((s) => s?.general?.auto_check_updates && Bridge.invoke("check_for_update"))
      .then((u) => { if (u?.update_available) UI.toast(`MelloGram ${u.name || u.tag} is available — Settings → About & Updates`, "info"); })
      .catch(() => {});
    await refreshChats();
  }

  // ---- chats ------------------------------------------------------------------

  const AVATAR_COLORS = ["#e17076", "#7bc862", "#e5ca77", "#65aadd", "#a695e7", "#ee7aae", "#6ec9cb", "#faa774"];
  function avatar(name, id, photo) {
    const initials = (name || "?").split(/\s+/).map((w) => [...w][0] || "").join("").slice(0, 2).toUpperCase();
    const el = h("div", { class: "mg-avatar", style: `background:${AVATAR_COLORS[Math.abs(Number(id)) % AVATAR_COLORS.length]}`, text: initials });
    if (photo) Photos.attach(el, id, photo);
    return el;
  }

  // Profile photos: downloaded by the backend (small 160px version), cached on
  // disk by photo id, loaded lazily as avatars scroll into view, at most four
  // downloads at a time. Initials stay visible until (and unless) a photo loads.
  const Photos = (() => {
    const urls = new Map();   // "chat:photo" -> Promise<url|null>
    const queue = [];
    let running = 0;
    const pump = () => {
      while (running < 4 && queue.length) {
        const job = queue.shift();
        running++;
        Bridge.invoke("chat_photo", { chatId: job.id, photoId: job.photo })
          .catch(() => null)
          .then((u) => { running--; job.resolve(u); pump(); });
      }
    };
    const get = (id, photo) => {
      const key = `${id}:${photo}`;
      if (!urls.has(key)) urls.set(key, new Promise((resolve) => { queue.push({ id, photo, resolve }); pump(); }));
      return urls.get(key);
    };
    const load = async (el) => {
      const url = await get(el.dataset.photoChat, el.dataset.photo);
      if (!url || !el.isConnected) return;
      const img = h("img", { class: "mg-avatar-img", alt: "", src: url, draggable: false });
      img.addEventListener("load", () => el.classList.add("mg-has-photo"));
      img.addEventListener("error", () => img.remove());
      el.append(img);
    };
    const io = "IntersectionObserver" in window
      ? new IntersectionObserver((entries) => {
        for (const e of entries) if (e.isIntersecting) { io.unobserve(e.target); load(e.target); }
      }, { rootMargin: "200px" })
      : null;
    return {
      attach(el, id, photo) {
        el.dataset.photoChat = String(id);
        el.dataset.photo = String(photo);
        if (io) io.observe(el); else load(el);
      },
    };
  })();

  async function refreshChats() {
    try {
      state.chats = await Bridge.invoke("list_chats", { limit: 300 });
      renderChats();
    } catch (e) {
      UI.toast(String(e), "error");
    }
  }

  function shortTime(seconds) {
    if (!seconds) return "";
    const d = new Date(seconds * 1000);
    const now = new Date();
    return d.toDateString() === now.toDateString()
      ? d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })
      : d.toLocaleDateString([], { month: "short", day: "numeric" });
  }

  function renderChats() {
    const q = $("chat-search").value.trim().toLowerCase();
    let chats = state.chats.filter((c) => !q || c.title.toLowerCase().includes(q) || (c.username || "").toLowerCase().includes(q));
    const sorters = [...Hooks.chatSorters];
    if (sorters.length) {
      chats = chats.map((c, i) => [c, i]).sort(([a, ia], [b, ib]) => {
        for (const s of sorters) { const r = s(a, b); if (r) return r; }
        return ia - ib;
      }).map(([c]) => c);
    }
    $("chat-list").replaceChildren(...chats.map((c) => {
      const el = h("li", {
        class: `mg-chat-item${c.pinned ? " mg-pinned" : ""}${state.current?.id === c.id ? " mg-selected" : ""}`,
        "data-chat-id": c.id,
        onclick: () => openChat(c),
        oncontextmenu: (e) => { e.preventDefault(); chatMenu(e, c); },
      },
      avatar(c.title, c.id, c.photo),
      h("div", { class: "mg-chat-meta" },
        h("div", { class: "mg-chat-top" }, h("span", { class: "mg-chat-name", text: c.title }), h("span", { class: "mg-chat-time", text: shortTime(c.last_date) })),
        h("div", { class: "mg-chat-last", text: c.last_text || "" })),
      c.unread ? h("span", { class: "mg-unread", text: c.unread > 999 ? "999+" : c.unread }) : null);
      for (const fn of Hooks.chatRender) { try { fn(c, el); } catch (e) { console.error(e); } }
      return el;
    }));
  }
  $("chat-search").addEventListener("input", renderChats);

  function chatMenu(e, chat) {
    const items = [
      { label: "Mark as read", onClick: () => Bridge.invoke("mark_read", { chatId: chat.id }).then(() => { chat.unread = 0; renderChats(); }) },
      { label: "Copy ID", onClick: () => navigator.clipboard.writeText(String(chat.id)) },
    ];
    for (const a of Hooks.chatActions.values()) {
      items.push({ label: typeof a.label === "function" ? a.label(chat) : a.label, onClick: () => a.onClick(chat) });
    }
    UI.contextMenu(e.clientX, e.clientY, items);
  }

  async function openChat(chat) {
    state.current = chat;
    state.replyTo = null;
    $("reply-bar").hidden = true;
    $("empty-state").hidden = true;
    $("chat-view").hidden = false;
    $("chat-view").dataset.chatId = chat.id;
    $("app").classList.add("mg-show-chat");
    $("chat-title").textContent = chat.title;
    $("chat-sub").textContent = [chat.username && `@${chat.username}`, chat.kind].filter(Boolean).join(" · ");
    window.MelloGram.E2E?.sync(chat);
    renderChats();
    if (!state.messages.has(chat.id)) {
      state.messages.set(chat.id, new Map());
      await loadOlder(true);
    } else {
      renderMessages(true);
    }
    for (const fn of Hooks.chatOpen) { try { fn(chat); } catch (e) { console.error(e); } }
    if (chat.unread) {
      Bridge.invoke("mark_read", { chatId: chat.id }).catch(() => {});
      chat.unread = 0;
      renderChats();
    }
    if (!document.body.classList.contains("mg-mobile")) $("composer").focus();
  }

  $("chat-back").addEventListener("click", () => $("app").classList.remove("mg-show-chat"));

  let loadingOlder = false;
  async function loadOlder(scrollToEnd = false) {
    const chat = state.current;
    if (!chat || loadingOlder) return;
    loadingOlder = true;
    const map = state.messages.get(chat.id);
    const oldest = map.size ? Math.min(...map.keys()) : undefined;
    try {
      const older = await Bridge.invoke("load_history", { chatId: chat.id, before: oldest });
      if (state.current?.id !== chat.id) return;
      const box = $("messages");
      const prevHeight = box.scrollHeight;
      const merged = new Map([...older.map((m) => [m.id, m]), ...map]);
      state.messages.set(chat.id, merged);
      renderMessages(scrollToEnd);
      if (!scrollToEnd) box.scrollTop += box.scrollHeight - prevHeight;
    } catch (e) {
      UI.toast(String(e), "error");
    } finally {
      loadingOlder = false;
    }
  }
  $("messages").addEventListener("scroll", (e) => { if (e.target.scrollTop < 80) loadOlder(); });

  // ---- messages ------------------------------------------------------------------

  function formatTime(seconds) {
    if (Hooks.timeFormatter) { try { return Hooks.timeFormatter(seconds); } catch (e) { console.error(e); } }
    return new Date(seconds * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  }

  /** Minimal, safe formatting: ```code```, `inline`, and clickable links. */
  function renderText(text) {
    const frag = document.createDocumentFragment();
    const parts = text.split(/(```[\s\S]*?```)/g);
    for (const part of parts) {
      if (part.startsWith("```") && part.endsWith("```") && part.length >= 6) {
        const body = part.slice(3, -3).replace(/^[\w+-]*\n/, "");
        frag.append(h("pre", {}, h("code", { text: body })));
        continue;
      }
      for (const seg of part.split(/(`[^`\n]+`)/g)) {
        if (/^`[^`\n]+`$/.test(seg)) { frag.append(h("code", { text: seg.slice(1, -1) })); continue; }
        for (const bit of seg.split(/(https?:\/\/[^\s<>"']+)/g)) {
          if (/^https?:\/\//.test(bit)) frag.append(h("a", { href: bit, target: "_blank", rel: "noopener noreferrer", text: bit }));
          else if (bit) frag.append(bit);
        }
      }
    }
    return frag;
  }

  function messageEl(m, map) {
    const reply = m.reply_to && map.get(m.reply_to);
    const classes = ["mg-message", m.outgoing && "mg-out", m.deleted && "mg-deleted", freshIds.has(m.id) && "mg-new",
      ...(m.classes || []).filter((c) => /^[a-z0-9-]+$/.test(c))].filter(Boolean).join(" ");
    const el = h("div", {
      class: classes,
      "data-id": m.id,
      "data-sender-id": m.sender_id ?? "",
      oncontextmenu: (e) => { e.preventDefault(); messageMenu(e, m); },
      ondblclick: () => setReply(m),
    },
    h("div", { class: "mg-sender" },
      !m.outgoing && m.sender_id != null && state.current?.kind !== "user"
        ? Object.assign(avatar(m.display_name || m.sender_name || "?", m.sender_id, m.sender_photo), { className: "mg-avatar mg-sender-avatar" })
        : null,
      m.display_name || m.sender_name || ""),
    m.reply_to ? h("div", { class: "mg-reply-preview", text: reply ? (reply.text || "Media").slice(0, 90) : "Reply", onclick: () => jumpTo(m.reply_to) }) : null,
    m.edit_history?.length ? h("div", { class: "mg-edit-history" }, ...m.edit_history.map((t) => h("div", { text: t }))) : null,
    m.__album ? window.MelloGram.Media.album(m.__album) : (m.media || m.has_media) && window.MelloGram.Media ? window.MelloGram.Media.render(m) : null,
    h("div", { class: "mg-text" }, renderText(m.text)),
    h("div", { class: "mg-meta" },
      ...m.badges.map((b) => h("span", { class: "mg-badge", title: b.tooltip || b.plugin, text: b.label })),
      m.deleted ? h("span", { class: "mg-badge", text: "deleted" }) : null,
      m.silent ? h("span", { title: "Sent silently", text: "🔕" }) : null,
      h("span", { class: "mg-time", title: new Date(m.date * 1000).toLocaleString(), text: formatTime(m.date) })));
    if (m.highlight && CSS.supports("color", m.highlight)) el.style.setProperty("--mg-hl", m.highlight);
    for (const fn of Hooks.messageRender) { try { fn(m, el); } catch (e) { console.error(e); } }
    return el;
  }

  function renderMessages(scrollToEnd) {
    const chat = state.current;
    if (!chat) return;
    const box = $("messages");
    const atBottom = box.scrollHeight - box.scrollTop - box.clientHeight < 60;
    const map = state.messages.get(chat.id) || new Map();
    const sorted = [...map.values()].sort((a, b) => a.id - b.id);
    const nodes = [];
    let lastDay = "";
    for (let i = 0; i < sorted.length; i++) {
      let m = sorted[i];
      const day = new Date(m.date * 1000).toDateString();
      if (day !== lastDay) { nodes.push(h("div", { class: "mg-day", text: new Date(m.date * 1000).toLocaleDateString([], { weekday: "long", month: "long", day: "numeric" }) })); lastDay = day; }
      // Albums: consecutive messages sharing a grouped_id render as one bubble.
      if (m.grouped_id) {
        const group = [m];
        while (i + 1 < sorted.length && sorted[i + 1].grouped_id === m.grouped_id) group.push(sorted[++i]);
        if (group.length > 1) {
          const caption = group.find((g) => g.text) || group[group.length - 1];
          m = { ...caption, __album: group, media: null, has_media: false };
        }
      }
      nodes.push(messageEl(m, map));
    }
    box.replaceChildren(...nodes);
    if (scrollToEnd || atBottom) box.scrollTop = box.scrollHeight;
  }

  function jumpTo(id) {
    const el = $("messages").querySelector(`[data-id="${id}"]`);
    if (!el) return;
    el.scrollIntoView({ block: "center", behavior: "smooth" });
    el.classList.add("mg-flash");
    setTimeout(() => el.classList.remove("mg-flash"), 1200);
  }

  function messageMenu(e, m) {
    const items = [
      { label: "Reply", onClick: () => setReply(m) },
      { label: "Copy text", onClick: () => navigator.clipboard.writeText(m.text) },
    ];
    if (m.outgoing && !m.deleted) items.push({ label: "Edit", onClick: () => editMessage(m) });
    for (const a of Hooks.messageActions.values()) items.push({ label: a.label, onClick: () => a.onClick(structuredClone(m)) });
    if (!m.deleted) items.push({ label: "Delete", danger: true, onClick: () => deleteMessage(m) });
    UI.contextMenu(e.clientX, e.clientY, items);
  }

  function setReply(m) {
    state.replyTo = m;
    $("reply-text").textContent = `Replying to ${m.display_name || m.sender_name || "message"}: ${m.text.slice(0, 120)}`;
    $("reply-bar").hidden = false;
    $("composer").focus();
  }
  $("reply-cancel").addEventListener("click", () => { state.replyTo = null; $("reply-bar").hidden = true; });

  function editMessage(m) {
    const ta = h("textarea", { rows: 4 });
    ta.value = m.text;
    const close = UI.modal("Edit message", [ta, h("div", { class: "mg-toolbar" }, h("button", {
      class: "mg-btn mg-primary", text: "Save",
      onclick: async () => {
        try { await Bridge.invoke("edit_message", { chatId: m.chat_id, id: m.id, text: ta.value }); close(); }
        catch (e) { UI.toast(String(e), "error"); }
      },
    }))]);
    ta.focus();
  }

  async function deleteMessage(m) {
    if (!(await UI.confirm("Delete message?", "This deletes it for everyone where Telegram allows it.", "Delete"))) return;
    try { await Bridge.invoke("delete_messages", { chatId: m.chat_id, ids: [m.id] }); }
    catch (e) { UI.toast(String(e), "error"); }
  }

  // ---- composer --------------------------------------------------------------------

  const composer = $("composer");

  function autosize() {
    composer.style.height = "auto";
    composer.style.height = `${composer.scrollHeight + 2}px`;
  }

  composer.addEventListener("input", () => {
    autosize();
    for (const fn of Hooks.composerInput) { try { fn(composer.value); } catch (e) { console.error(e); } }
    updateCommandPopup();
    const now = Date.now();
    if (state.current && composer.value && now - state.lastTyping > 4000) {
      state.lastTyping = now;
      Bridge.invoke("typing", { chatId: state.current.id }).catch(() => {});
    }
  });

  function matchingCommands() {
    const m = /^\/([\w-]*)$/.exec(composer.value);
    if (!m) return [];
    return state.commands.filter((c) => c.name.startsWith(m[1]));
  }

  function updateCommandPopup() {
    const list = matchingCommands();
    const popup = $("command-popup");
    popup.hidden = !list.length;
    if (!list.length) return;
    state.commandIndex = Math.min(state.commandIndex, list.length - 1);
    popup.replaceChildren(...list.map((c, i) => h("div", {
      class: `mg-command${i === state.commandIndex ? " mg-active" : ""}`,
      onmousedown: (e) => { e.preventDefault(); pickCommand(c); },
    }, h("b", { text: `/${c.name}` }), h("span", { text: c.description }), h("small", { class: "mg-muted", text: c.plugin }))));
  }

  function pickCommand(c) {
    composer.value = `/${c.name} `;
    $("command-popup").hidden = true;
    composer.focus();
  }

  composer.addEventListener("keydown", (e) => {
    const list = $("command-popup").hidden ? [] : matchingCommands();
    if (list.length && (e.key === "ArrowDown" || e.key === "ArrowUp")) {
      e.preventDefault();
      state.commandIndex = (state.commandIndex + (e.key === "ArrowDown" ? 1 : list.length - 1)) % list.length;
      updateCommandPopup();
      return;
    }
    if (list.length && (e.key === "Tab" || e.key === "Enter")) { e.preventDefault(); pickCommand(list[state.commandIndex]); return; }
    if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); send(); }
    if (e.key === "Escape") { $("command-popup").hidden = true; }
  });
  $("send").addEventListener("click", send);

  async function send() {
    const chat = state.current;
    const text = composer.value;
    if (!chat || (!text.trim() && !state.attachments.length)) return;
    const request = {
      chat_id: chat.id,
      text,
      reply_to: state.replyTo?.id ?? null,
      silent: !!Hooks.composerFlags.silent,
      attachments: state.attachments.map((a) => a.path),
      schedule_at: Hooks.composerFlags.scheduleAt ?? null,
    };
    composer.value = "";
    autosize();
    $("command-popup").hidden = true;
    let result;
    try {
      result = await Bridge.invoke("send_message", { request });
    } catch (e) {
      composer.value = text;
      UI.toast(String(e), "error");
      return;
    }
    state.replyTo = null;
    $("reply-bar").hidden = true;
    state.attachments = [];
    renderAttachments();
    if (result.status === "cancelled") {
      if (result.reason) UI.toast(result.reason, "info");
    } else if (result.status === "scheduled") {
      UI.toast(`Scheduled for ${new Date(result.at * 1000).toLocaleString()}`, "success");
    } else {
      upsert(result.message);
    }
    for (const fn of Hooks.composerSent) { try { fn(result); } catch (e) { console.error(e); } }
    for (const fn of Hooks.composerInput) { try { fn(""); } catch (e) { console.error(e); } }
  }

  // Drag & drop attachments. Tauri gives us real paths through its drop event.
  function renderAttachments() {
    const box = $("attachments");
    box.hidden = !state.attachments.length;
    box.replaceChildren(...state.attachments.map((a, i) => h("span", { class: "mg-chip" }, `📎 ${a.name} `,
      h("button", { class: "mg-icon-btn", text: "✕", onclick: () => { state.attachments.splice(i, 1); renderAttachments(); } }))));
  }
  if (window.__TAURI__?.webview) {
    window.__TAURI__.webview.getCurrentWebview().onDragDropEvent((e) => {
      if (e.payload.type !== "drop" || !state.current) return;
      for (const path of e.payload.paths) state.attachments.push({ path, name: path.split(/[\\/]/).pop() });
      renderAttachments();
    });
  }

  // ---- live events -------------------------------------------------------------------

  // Messages that arrived live get the entry animation once.
  const freshIds = new Set();
  function upsert(m) {
    if (!state.messages.has(m.chat_id)) return;
    if (!state.messages.get(m.chat_id).has(m.id)) {
      freshIds.add(m.id);
      setTimeout(() => freshIds.delete(m.id), 600);
    }
    state.messages.get(m.chat_id).set(m.id, m);
    if (state.current?.id === m.chat_id) renderMessages(m.outgoing);
  }

  function findById(id) {
    for (const [chatId, map] of state.messages) if (map.has(id)) return [chatId, map];
    return null;
  }

  Bridge.listen("mg://event", (ev) => {
    switch (ev.type) {
      case "new-message": {
        const m = ev;
        upsert(m);
        const chat = state.chats.find((c) => c.id === m.chat_id);
        if (chat) {
          chat.last_text = m.text || (m.has_media ? "📎 Media" : "");
          chat.last_date = m.date;
          if (!m.outgoing && state.current?.id !== m.chat_id) chat.unread += 1;
          state.chats = [chat, ...state.chats.filter((c) => c !== chat)];
          renderChats();
        } else {
          refreshChats();
        }
        break;
      }
      case "edited": upsert(ev); break;
      case "deleted":
        for (const id of ev.ids) {
          const found = ev.chat_id != null ? [ev.chat_id, state.messages.get(ev.chat_id)] : findById(id);
          const map = found?.[1];
          if (!map?.has(id)) continue;
          if (ev.keep) map.get(id).deleted = true; else map.delete(id);
        }
        renderMessages(false);
        break;
      case "disconnected": UI.toast(`Disconnected: ${ev.reason}`, "warning"); break;
      case "logged-in": refreshChats(); break;
    }
  });
  Bridge.listen("mg://toast", (t) => UI.toast(t.message, t.level, t.plugin));
  Bridge.listen("mg://stylesheet", (css) => applyStylesheet(css));
  Bridge.listen("mg://plugins-changed", async () => {
    state.commands = await Bridge.invoke("list_commands");
    await PluginHost.sync(App);
  });

  // ---- touch: long-press opens context menus ------------------------------------------

  let pressTimer = null;
  document.addEventListener("touchstart", (e) => {
    const target = e.target.closest(".mg-message, .mg-chat-item");
    if (!target) return;
    const t = e.touches[0];
    pressTimer = setTimeout(() => {
      target.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, clientX: t.clientX, clientY: t.clientY }));
      navigator.vibrate?.(15);
    }, 480);
  }, { passive: true });
  for (const ev of ["touchend", "touchmove", "touchcancel"]) document.addEventListener(ev, () => clearTimeout(pressTimer), { passive: true });

  const mobileQuery = matchMedia("(max-width: 700px)");
  const applyMobile = () => document.body.classList.toggle("mg-mobile", mobileQuery.matches);
  mobileQuery.addEventListener("change", applyMobile);
  applyMobile();

  // ---- keyboard shortcuts -----------------------------------------------------------

  document.addEventListener("keydown", (e) => {
    if ((e.ctrlKey || e.metaKey) && e.key === ",") { e.preventDefault(); window.MelloGram.Settings.open(); }
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") { e.preventDefault(); $("chat-search").focus(); }
  });

  // ---- boot -------------------------------------------------------------------------

  async function boot() {
    await applyStylesheet();
    const settings = await Bridge.invoke("get_settings");
    UI.toastTimeout = settings.notifications.timeout_ms;
    $("toasts").className = `mg-toasts ${settings.notifications.position}`;
    state.commands = await Bridge.invoke("list_commands");
    await PluginHost.sync(App);
    // The backend connects in the background; give it a moment on cold start.
    let login = null;
    for (let i = 0; i < 20 && settings.telegram.has_hash; i++) {
      login = await Bridge.invoke("login_state").catch(() => null);
      if (login) break;
      await new Promise((r) => setTimeout(r, 250));
    }
    showLogin(login);
    window.MelloGram.Extras?.afterBoot(await Bridge.invoke("startup_status"));
  }
  // Wait until every script (settings, studio, extras) has loaded before booting.
  const start = () => boot().catch((e) => UI.toast(`Startup failed: ${e}`, "error"));
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", start);
  else start();
})();
