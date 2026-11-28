import { initials, senderLine, unreadBadge } from "./chat-format.js";

const byId = (id) => document.getElementById(id);
const panel = byId("chats-panel");
const chatsEl = byId("chat-list");
const status = byId("chat-status");
const messagesEl = byId("messages");
const heading = byId("chats-heading");
const reload = byId("reload-messages");
const older = byId("older-messages");
const reloadChats = byId("reload-chats");
const composer = byId("compose");
const draft = byId("compose-text");
const send = byId("send-message");
const signOut = byId("signout");
const forget = byId("forget-session");
const PAGE_SIZE = 30;
const MAX_HISTORY = 120;

export function openChats(client, sdk, user, onExit) {
  const lifetime = new AbortController();
  const { signal } = lifetime;
  let conversations = new Map();
  let selectedId = "";
  let view;
  let offset;
  let history = [];
  let loading = false;
  let loadingChats = false;
  let sending = false;
  let leaving = false;
  let downloading = false;
  let refreshTimer;
  let typingAt = -Infinity;

  const selected = () => conversations.get(selectedId);

  panel.hidden = false;
  byId("account-name").textContent = user.displayName;
  byId("account-avatar").textContent = initials(user.displayName);
  heading.tabIndex = -1;
  heading.focus();

  function buttons() {
    const busy = loadingChats || sending || leaving;
    for (const button of chatsEl.querySelectorAll("button")) button.disabled = busy;
    reloadChats.disabled = busy;
    reload.disabled = !selected() || loading || sending || leaving;
    older.disabled = reload.disabled || !offset || history.length >= MAX_HISTORY;
    send.disabled = !selected() || sending || leaving;
    signOut.disabled = leaving || sending;
    forget.disabled = leaving || sending;
    chatsEl.setAttribute("aria-busy", String(loadingChats));
  }

  function renderChats() {
    if (!conversations.size) {
      const empty = document.createElement("li");
      empty.className = "tg-sidebar-empty";
      empty.textContent = "No supported chats found.";
      chatsEl.replaceChildren(empty);
      return;
    }
    const rows = [...conversations.values()].map((chat) => {
      const item = document.createElement("li");
      const button = document.createElement("button");
      button.type = "button";
      button.className = "tg-chat";
      button.dataset.id = chat.id;
      if (chat.id === selectedId) button.setAttribute("aria-current", "true");

      const avatar = document.createElement("span");
      avatar.className = "tg-chat-avatar";
      avatar.setAttribute("aria-hidden", "true");
      avatar.textContent = initials(chat.displayName);

      const name = document.createElement("span");
      name.className = "tg-chat-name";
      name.textContent = chat.displayName;

      const badge = document.createElement("span");
      badge.className = "tg-unread";
      badge.textContent = unreadBadge(chat);
      badge.hidden = !badge.textContent;
      badge.setAttribute("aria-label", badge.textContent ? `${badge.textContent} unread` : "");

      button.append(avatar, name, badge);
      item.append(button);
      return item;
    });
    chatsEl.replaceChildren(...rows);
  }

  function renderHistory({ previous = false } = {}) {
    const nearBottom = messagesEl.scrollHeight - messagesEl.scrollTop - messagesEl.clientHeight < 80;
    const before = messagesEl.scrollHeight;
    const rows = history.slice().reverse().flatMap((message) => {
      // Ephemeral content and service events need dedicated presentation.
      if (message.isService || message.ttlPeriod || message.raw.media?.ttlSeconds) return [];
      const item = document.createElement("li");
      item.className = message.isOutgoing ? "tg-msg is-out" : "tg-msg is-in";
      const bubble = document.createElement("div");
      bubble.className = "tg-bubble";
      const meta = document.createElement("span");
      meta.className = "tg-msg-meta";
      meta.textContent = senderLine(message);
      const body = document.createElement("p");
      body.textContent = message.text;
      bubble.append(meta);
      if (message.text) bubble.append(body);

      const media = sdk.mediaFor(message);
      if (media) {
        const button = document.createElement("button");
        button.type = "button";
        button.className = "tg-download";
        button.textContent = `Download ${media.fileName || media.type || "attachment"}`;
        button.addEventListener("click", () => void save(media, button));
        bubble.append(button);
      } else if (message.media && !message.text) {
        const fallback = document.createElement("p");
        fallback.textContent = "Attachment unavailable in the web client.";
        bubble.append(fallback);
      }
      item.append(bubble);
      return [item];
    });
    if (!rows.length) {
      const empty = document.createElement("li");
      empty.className = "tg-empty";
      empty.textContent = "No messages here yet. Write the first one.";
      rows.push(empty);
    }
    messagesEl.replaceChildren(...rows);
    if (previous) messagesEl.scrollTop = messagesEl.scrollHeight - before;
    else if (nearBottom) messagesEl.scrollTop = messagesEl.scrollHeight;
  }

  async function save(media, button) {
    if (!view || downloading || leaving) return;
    const downloadSignal = AbortSignal.any([view.signal, AbortSignal.timeout(30000)]);
    downloading = true;
    button.disabled = true;
    status.textContent = "Downloading from Telegram…";
    try {
      const file = await sdk.downloadMedia(client, media, downloadSignal);
      downloadSignal.throwIfAborted();
      const url = URL.createObjectURL(file);
      const link = document.createElement("a");
      link.href = url;
      link.download = (media.fileName || "telegram-attachment").replace(/[\\/\x00-\x1f]/g, "_");
      link.click();
      setTimeout(() => URL.revokeObjectURL(url), 1000);
      status.textContent = "Download ready.";
    } catch {
      if (!signal.aborted && !view?.signal.aborted && !downloadSignal.aborted) {
        status.textContent = "Could not download this attachment. The limit is 4 MiB.";
      } else if (downloadSignal.reason?.name === "TimeoutError") {
        status.textContent = "The download timed out.";
      }
    } finally {
      downloading = false;
      button.disabled = false;
    }
  }

  async function loadMessages({ previous = false, quiet = false } = {}) {
    const chat = selected();
    if (!chat || loading || leaving) return;
    const current = view;
    loading = true;
    buttons();
    if (!quiet) status.textContent = "Loading messages…";
    try {
      // Refresh the loaded window so edits and deletions are reflected without
      // collapsing the older pages the user opened.
      const limit = previous ? Math.min(PAGE_SIZE, MAX_HISTORY - history.length)
        : Math.max(PAGE_SIZE, history.length);
      let page = await client.getHistory(chat.inputPeer, { limit: Math.min(limit, 100), offset: previous ? offset : undefined });
      if (current.signal.aborted || leaving) return;
      const oldest = history.at(-1)?.id;
      while (!previous && page.next && page.length < MAX_HISTORY && (
        page.length < limit || (limit > PAGE_SIZE && page.at(-1)?.id > oldest)
      )) {
        const remaining = page.length < limit ? limit - page.length : PAGE_SIZE;
        const tail = await client.getHistory(chat.inputPeer, {
          limit: Math.min(remaining, MAX_HISTORY - page.length), offset: page.next,
        });
        if (current.signal.aborted || leaving) return;
        const newRows = tail.filter((message) => message.id < page.at(-1)?.id);
        page = Object.assign([...page, ...newRows], { next: tail.next });
        if (!newRows.length) break;
      }
      if (current.signal.aborted || leaving) return;
      const merged = previous ? [...history, ...page] : page;
      history = [...new Map(merged.map((message) => [message.id, message])).values()];
      offset = page.next;
      renderHistory({ previous });
      if (!quiet) status.textContent = "";
      if (!previous && document.visibilityState === "visible" && page.length) {
        // Only acknowledge a conversation that the user actually opened.
        void client.readHistory(chat.inputPeer, { maxId: page[0].id }).catch(() => {});
      }
    } catch (error) {
      if (current.signal.aborted || leaving) return;
      if (sdk.sessionExpired(error)) return close("Your Telegram session ended. Sign in again.");
      if (!quiet) status.textContent = sdk.telegramError(error, "Could not load messages. Try refreshing.");
    } finally {
      if (current === view) {
        loading = false;
        buttons();
      }
    }
  }

  function choose(id = "") {
    view?.abort();
    view = new AbortController();
    selectedId = conversations.has(id) ? id : "";
    const chat = selected();
    typingAt = -Infinity;
    loading = false;
    offset = undefined;
    history = [];
    messagesEl.replaceChildren();
    composer.reset();
    composer.hidden = !chat;
    heading.textContent = chat ? chat.displayName : "Choose a chat";
    renderChats();
    buttons();
    if (chat) void loadMessages();
    else status.textContent = "";
  }

  async function loadChats({ quiet = false } = {}) {
    if (loadingChats || leaving) return;
    const wanted = selectedId;
    loadingChats = true;
    buttons();
    if (!quiet) status.textContent = "Loading chats…";
    try {
      const chats = await sdk.listChats(client);
      if (signal.aborted || leaving) return;
      conversations = new Map(chats.map((chat) => [String(chat.id), chat]));
      if (conversations.has(wanted)) {
        renderChats();
        if (!quiet && !history.length) status.textContent = "";
      } else {
        choose();
        if (!quiet) status.textContent = chats.length ? "" : "No supported chats found.";
      }
    } catch (error) {
      if (signal.aborted) return;
      if (sdk.sessionExpired(error)) return close("Your Telegram session ended. Sign in again.");
      if (!quiet) status.textContent = sdk.telegramError(error, "Could not load chats. Try refreshing.");
    } finally {
      loadingChats = false;
      if (!signal.aborted) buttons();
    }
  }

  function refreshSoon() {
    if (signal.aborted || !selected() || document.visibilityState !== "visible") return;
    clearTimeout(refreshTimer);
    refreshTimer = setTimeout(() => {
      if (loading || sending) return refreshSoon();
      void loadMessages({ quiet: true });
    }, 600);
  }
  const onMessage = (message) => { if (message.chat.id === selectedId) refreshSoon(); };
  client.onNewMessage.add(onMessage);
  client.onEditMessage.add(onMessage);
  client.onDeleteMessage.add(refreshSoon);

  // Unread counts change in chats the user is not looking at. Refresh the list
  // occasionally instead of on every message, and never while it is focused.
  let chatsTimer;
  const onAnyMessage = () => {
    if (signal.aborted || document.visibilityState !== "visible") return;
    clearTimeout(chatsTimer);
    chatsTimer = setTimeout(() => {
      if (signal.aborted || loadingChats || sending || chatsEl.contains(document.activeElement)) return;
      void loadChats({ quiet: true });
    }, 8000);
  };
  client.onNewMessage.add(onAnyMessage);

  function visibility() {
    if (document.visibilityState === "visible") refreshSoon();
    void client.setOnline(document.visibilityState === "visible").catch(() => {});
  }
  document.addEventListener("visibilitychange", visibility, { signal });
  visibility();
  chatsEl.addEventListener("click", (event) => {
    const button = event.target.closest("button[data-id]");
    if (!button || button.disabled || button.dataset.id === selectedId) return;
    choose(button.dataset.id);
  }, { signal });
  reloadChats.addEventListener("click", () => void loadChats(), { signal });
  reload.addEventListener("click", () => void loadMessages(), { signal });
  older.addEventListener("click", () => void loadMessages({ previous: true }), { signal });
  draft.addEventListener("input", () => {
    if (!selected() || !draft.value || leaving || performance.now() - typingAt < 5000) return;
    typingAt = performance.now();
    void client.sendTyping(selected().inputPeer, "typing").catch(() => {});
  }, { signal });
  draft.addEventListener("keydown", (event) => {
    if (event.key !== "Enter" || event.shiftKey || event.isComposing) return;
    event.preventDefault();
    composer.requestSubmit();
  }, { signal });
  composer.addEventListener("submit", async (event) => {
    event.preventDefault();
    const chat = selected();
    if (!chat || leaving || sending || !draft.value.trim()) return;
    const text = draft.value;
    sending = true;
    buttons();
    status.textContent = "Sending…";
    try {
      await client.sendText(chat.inputPeer, text, { disableWebPreview: true, abortSignal: AbortSignal.timeout(30000) });
      if (draft.value === text) draft.value = "";
      status.textContent = "Sent.";
      refreshSoon();
    } catch (error) {
      status.textContent = sdk.sessionExpired(error)
        ? "Your Telegram session ended. Sign in again."
        : sdk.telegramError(error, "Telegram did not confirm delivery. Refresh messages before trying again.");
    } finally {
      sending = false;
      buttons();
    }
  }, { signal });

  function close(message) {
    lifetime.abort();
    view?.abort();
    clearTimeout(refreshTimer);
    clearTimeout(chatsTimer);
    client.onNewMessage.remove(onMessage);
    client.onEditMessage.remove(onMessage);
    client.onDeleteMessage.remove(refreshSoon);
    client.onNewMessage.remove(onAnyMessage);
    byId("account-name").textContent = "";
    byId("account-avatar").textContent = "M";
    status.textContent = "";
    chatsEl.replaceChildren();
    messagesEl.replaceChildren();
    composer.reset();
    heading.textContent = "Choose a chat";
    panel.hidden = true;
    onExit(message);
  }

  signOut.addEventListener("click", async () => {
    if (leaving || sending) return;
    leaving = true;
    buttons();
    status.textContent = "Signing out of Telegram…";
    try {
      await client.logOut();
      // Revoking happens first, then the connection is closed, and only then is
      // the database deleted: an open client blocks the deletion and would leave
      // the authorization key on disk after a sign-out.
      await client.destroy().catch(() => {});
      await sdk.forgetDevice().catch(() => {});
      close("Signed out of Telegram.");
    } catch {
      leaving = false;
      buttons();
      status.textContent = "Telegram did not confirm sign-out. Retry, or use Forget this browser and end the session in Telegram Settings → Devices.";
    }
  }, { signal });
  forget.addEventListener("click", async () => {
    if (leaving || sending) return;
    leaving = true;
    buttons();
    status.textContent = "Clearing this browser…";
    // Close the connection before deleting the database, or the deletion is
    // blocked by the open client and the session survives the "clear".
    await client.destroy().catch(() => {});
    await sdk.forgetDevice().catch(() => {});
    close("This browser was cleared. If the session was not signed out, end it in Telegram Settings → Devices.");
  }, { signal });

  choose();
  void loadChats();
}
