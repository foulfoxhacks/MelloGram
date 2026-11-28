const byId = (id) => document.getElementById(id);
const panel = byId("chats-panel");
const select = byId("chat-list");
const status = byId("chat-status");
const list = byId("messages");
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
  let selected;
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

  panel.hidden = false;
  byId("account-name").textContent = `Signed in as ${user.displayName}`;
  byId("chats-heading").tabIndex = -1;
  byId("chats-heading").focus();

  function buttons() {
    select.disabled = loadingChats || sending || leaving;
    reloadChats.disabled = select.disabled;
    reload.disabled = !selected || loading || sending || leaving;
    older.disabled = reload.disabled || !offset || history.length >= MAX_HISTORY;
    send.disabled = !selected || sending || leaving;
    signOut.disabled = leaving || sending;
    forget.disabled = leaving || sending;
  }

  function renderHistory() {
    const rows = history.slice().reverse().flatMap((message) => {
      // Ephemeral content and service events need dedicated presentation.
      if (message.isService || message.ttlPeriod || message.raw.media?.ttlSeconds) return [];
      const item = document.createElement("li");
      const meta = document.createElement("span");
      const body = document.createElement("p");
      meta.className = "small muted";
      meta.textContent = `${message.isOutgoing ? "You" : message.sender.displayName} · ${message.date.toLocaleString()}`;
      body.textContent = message.text;
      item.append(meta);
      if (message.text) item.append(body);

      const media = sdk.mediaFor(message);
      if (media) {
        const button = document.createElement("button");
        button.type = "button";
        button.className = "tg-btn-ghost";
        button.textContent = `Download ${media.fileName || media.type || "attachment"}`;
        button.addEventListener("click", () => void save(media, button));
        item.append(button);
      } else if (message.media && !message.text) {
        body.textContent = "Attachment unavailable in the web client.";
        item.append(body);
      }
      return [item];
    });
    list.replaceChildren(...rows);
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
    if (!selected || loading || leaving) return;
    const chat = selected;
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
      renderHistory();
      if (!quiet) status.textContent = list.childElementCount ? "" : "No messages to show.";
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

  function choose() {
    view?.abort();
    view = new AbortController();
    selected = conversations.get(select.value);
    typingAt = -Infinity;
    loading = false;
    offset = undefined;
    history = [];
    list.replaceChildren();
    composer.reset();
    composer.hidden = !selected;
    buttons();
    if (selected) void loadMessages();
    else status.textContent = "";
  }

  async function loadChats({ quiet = false } = {}) {
    if (loadingChats || leaving) return;
    const old = select.value;
    loadingChats = true;
    buttons();
    if (!quiet) status.textContent = "Loading chats…";
    try {
      const chats = await sdk.listChats(client);
      if (signal.aborted || leaving) return;
      conversations = new Map(chats.map((chat) => [String(chat.id), chat]));
      select.replaceChildren(new Option("Select a conversation", ""), ...chats.map((chat) => new Option(
        chat.unreadCount ? `${chat.displayName} · ${chat.unreadCount} unread` : chat.displayName,
        String(chat.id),
      )));
      if (conversations.has(old)) select.value = old;
      else choose();
      if (!quiet) status.textContent = chats.length ? "" : "No supported chats found.";
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
    if (signal.aborted || !selected || document.visibilityState !== "visible") return;
    clearTimeout(refreshTimer);
    refreshTimer = setTimeout(() => {
      if (loading || sending) return refreshSoon();
      void loadMessages({ quiet: true });
    }, 600);
  }
  const onMessage = (message) => { if (message.chat.id === selected?.id) refreshSoon(); };
  client.onNewMessage.add(onMessage);
  client.onEditMessage.add(onMessage);
  client.onDeleteMessage.add(refreshSoon);

  // Unread counts change in chats the user is not looking at. Refresh the list
  // occasionally instead of on every message, and never while it is open.
  let chatsTimer;
  const onAnyMessage = () => {
    if (signal.aborted || document.visibilityState !== "visible") return;
    clearTimeout(chatsTimer);
    chatsTimer = setTimeout(() => {
      if (signal.aborted || loadingChats || sending || document.activeElement === select) return;
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
  select.addEventListener("change", choose, { signal });
  reloadChats.addEventListener("click", () => void loadChats(), { signal });
  reload.addEventListener("click", () => void loadMessages(), { signal });
  older.addEventListener("click", () => void loadMessages({ previous: true }), { signal });
  draft.addEventListener("input", () => {
    if (!selected || !draft.value || leaving || performance.now() - typingAt < 5000) return;
    typingAt = performance.now();
    void client.sendTyping(selected.inputPeer, "typing").catch(() => {});
  }, { signal });
  composer.addEventListener("submit", async (event) => {
    event.preventDefault();
    if (!selected || leaving || sending || !draft.value.trim()) return;
    const text = draft.value;
    sending = true;
    buttons();
    status.textContent = "Sending…";
    try {
      await client.sendText(selected.inputPeer, text, { disableWebPreview: true, abortSignal: AbortSignal.timeout(30000) });
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
    status.textContent = "";
    select.replaceChildren();
    list.replaceChildren();
    composer.reset();
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
      close("Signed out of Telegram.");
      void sdk.forgetDevice();
    } catch {
      leaving = false;
      buttons();
      status.textContent = "Telegram did not confirm sign-out. Retry, or use Forget this browser and end the session in Telegram Settings → Devices.";
    }
  }, { signal });
  forget.addEventListener("click", () => {
    if (leaving || sending) return;
    close("This browser was cleared. If the session was not signed out, end it in Telegram Settings → Devices.");
    void sdk.forgetDevice();
  }, { signal });

  choose();
  void loadChats();
}
