// MelloGram frontend runtime: backend bridge, UI helpers and the `MG` API
// handed to plugin UI scripts. See docs/PLUGIN_API.md.
"use strict";

const Bridge = (() => {
  const T = window.__TAURI__;
  const mock = window.MG_MOCK; // only present in the browser preview harness
  return {
    invoke: (cmd, args) => (T ? T.core.invoke(cmd, args) : mock.invoke(cmd, args)),
    listen: (event, fn) => (T ? T.event.listen(event, (e) => fn(e.payload)) : mock.listen(event, fn)),
  };
})();

// ---- small DOM helpers ------------------------------------------------------

function h(tag, props = {}, ...children) {
  const el = document.createElement(tag);
  for (const [k, v] of Object.entries(props)) {
    if (v == null || v === false) continue;
    if (k === "class") el.className = v;
    else if (k === "text") el.textContent = v;
    else if (k.startsWith("on")) el.addEventListener(k.slice(2), v);
    else if (k in el && typeof v !== "string") el[k] = v;
    else el.setAttribute(k, v === true ? "" : v);
  }
  for (const c of children.flat()) if (c != null && c !== false) el.append(c instanceof Node ? c : String(c));
  return el;
}

const UI = {
  toast(message, level = "info", plugin) {
    const box = document.getElementById("toasts");
    const el = h("div", { class: `mg-toast ${level}` }, plugin ? h("small", { text: plugin }) : null, message);
    box.append(el);
    setTimeout(() => el.remove(), UI.toastTimeout || 5000);
  },

  modal(title, content) {
    const overlay = h("div", { class: "mg-overlay" });
    const close = () => { overlay.remove(); document.removeEventListener("keydown", onKey); };
    const onKey = (e) => { if (e.key === "Escape") close(); };
    const card = h("div", { class: "mg-card mg-modal" },
      h("div", { class: "mg-modal-head" }, h("h3", { text: title }), h("button", { class: "mg-icon-btn", text: "✕", onclick: close })),
      ...(Array.isArray(content) ? content : [content]).map((c) => (c instanceof Node ? c : h("p", { text: String(c) }))));
    overlay.append(card);
    overlay.addEventListener("click", (e) => { if (e.target === overlay) close(); });
    document.addEventListener("keydown", onKey);
    document.body.append(overlay);
    return close;
  },

  confirm(title, text, okLabel = "OK") {
    return new Promise((resolve) => {
      let close;
      const done = (v) => { close(); resolve(v); };
      close = UI.modal(title, [h("p", { text }), h("div", { class: "mg-toolbar" },
        h("button", { class: "mg-btn mg-primary", text: okLabel, onclick: () => done(true) }),
        h("button", { class: "mg-btn", text: "Cancel", onclick: () => done(false) }))]);
    });
  },

  contextMenu(x, y, items) {
    const menu = document.getElementById("context-menu");
    menu.replaceChildren(...items.map((it) => h("button", {
      class: it.danger ? "mg-danger" : "", text: it.label,
      onclick: () => { menu.hidden = true; it.onClick(); },
    })));
    menu.hidden = false;
    const r = menu.getBoundingClientRect();
    menu.style.left = `${Math.min(x, innerWidth - r.width - 8)}px`;
    menu.style.top = `${Math.min(y, innerHeight - r.height - 8)}px`;
  },
};
document.addEventListener("click", () => { document.getElementById("context-menu").hidden = true; });

// ---- extension points shared by the app and plugins ---------------------------

const Hooks = {
  composerInput: new Set(),
  composerSent: new Set(),
  messageActions: new Map(),
  messageRender: new Set(),
  chatSorters: new Set(),
  chatRender: new Set(),
  chatActions: new Map(),
  chatOpen: new Set(),
  timeFormatter: null,
  composerFlags: {},
};

/** Builds the `MG` object for one plugin. Everything it registers is tracked
 *  so disabling the plugin removes it even if the script forgets to. */
function createPluginApi(id, settings, app) {
  const cleanups = [];
  const track = (fn) => { cleanups.push(fn); return fn; };
  const addTo = (set, value) => { set.add(value); return track(() => set.delete(value)); };
  const button = (container, { id: bid, label, title, onClick }) => {
    const el = h("button", { class: "mg-icon-btn", "data-plugin": id, "data-id": bid, title, text: label, onclick: onClick });
    container.append(el);
    track(() => el.remove());
    return el;
  };

  const MG = {
    id,
    settings,
    invoke: Bridge.invoke,
    toast: (m, level) => UI.toast(m, level, id),
    modal: UI.modal,
    h,
    async setSetting(key, value) {
      await Bridge.invoke("set_plugin_setting", { id, key, value });
      settings[key] = value;
    },
    /** Per-plugin JSON storage on this device (needs the `storage` permission). */
    storage: {
      get: () => Bridge.invoke("plugin_storage_get", { id }),
      set: (value) => Bridge.invoke("plugin_storage_set", { id, value }),
    },
    clipboard: { write: (text) => navigator.clipboard.writeText(text) },
    /** Subscribe to backend events of one type (e.g. "membership"); auto-removed on disable. */
    onEvent(type, fn) {
      let off = null;
      let dead = false;
      Bridge.listen("mg://event", (ev) => { if (!dead && ev.type === type) fn(ev); }).then((u) => { off = u; if (dead) u?.(); });
      return track(() => { dead = true; off?.(); });
    },
    download(name, text, mime = "text/plain") {
      const url = URL.createObjectURL(new Blob([text], { type: mime }));
      h("a", { href: url, download: name }).click();
      setTimeout(() => URL.revokeObjectURL(url), 2000);
    },
    composer: {
      get el() { return document.getElementById("composer"); },
      get footer() { return document.getElementById("composer-footer"); },
      flags: Hooks.composerFlags,
      addButton: (o) => button(document.getElementById("composer-buttons"), o),
      onInput: (fn) => addTo(Hooks.composerInput, fn),
      onSent: (fn) => addTo(Hooks.composerSent, fn),
      /** Inserts at the caret; `{cursor}` marks where the caret ends up. */
      insert(text) {
        const el = document.getElementById("composer");
        const [before, after = ""] = text.split("{cursor}");
        el.setRangeText(before + after, el.selectionStart, el.selectionEnd, "end");
        el.selectionStart = el.selectionEnd = el.selectionEnd - after.length;
        el.focus();
        el.dispatchEvent(new Event("input"));
      },
    },
    header: { addButton: (o) => button(document.getElementById("header-buttons"), o) },
    messages: {
      addAction: (action) => {
        const key = `${id}:${action.id}`;
        Hooks.messageActions.set(key, action);
        return track(() => Hooks.messageActions.delete(key));
      },
      onRender: (fn) => { const off = addTo(Hooks.messageRender, fn); app.rerenderMessages(); return off; },
      setTimeFormatter: (fn) => {
        Hooks.timeFormatter = fn;
        app.rerenderMessages();
        return track(() => { if (Hooks.timeFormatter === fn) { Hooks.timeFormatter = null; app.rerenderMessages(); } });
      },
      rerender: () => app.rerenderMessages(),
      /** Messages loaded for the open chat, oldest first (copies). */
      loaded: () => structuredClone(app.loadedMessages()),
      jumpTo: (msgId) => app.jumpTo(msgId),
    },
    chats: {
      addSorter: (fn) => addTo(Hooks.chatSorters, fn),
      onRender: (fn) => addTo(Hooks.chatRender, fn),
      addAction: (action) => {
        const key = `${id}:${action.id}`;
        Hooks.chatActions.set(key, action);
        return track(() => Hooks.chatActions.delete(key));
      },
      rerender: () => app.renderChats(),
      refresh: () => app.refreshChats(),
      current: () => app.currentChat(),
      open: (chatId) => app.openChatById(chatId),
      onOpen: (fn) => addTo(Hooks.chatOpen, fn),
    },
  };
  const dispose = () => { while (cleanups.length) { try { cleanups.pop()(); } catch (e) { console.error(e); } } };
  return { MG, dispose };
}

/** Starts, stops and restarts plugin UI scripts to match the backend. */
const PluginHost = {
  running: new Map(), // id -> { dispose, key }

  async sync(app) {
    let wanted;
    try { wanted = await Bridge.invoke("ui_plugins"); } catch (e) { console.error(e); return; }
    const ids = new Set(wanted.map((p) => p.id));
    for (const [id, r] of this.running) if (!ids.has(id)) { r.dispose(); this.running.delete(id); }
    for (const p of wanted) {
      const key = JSON.stringify(p.settings);
      const current = this.running.get(p.id);
      if (current?.key === key) continue;
      current?.dispose();
      const { MG, dispose } = createPluginApi(p.id, structuredClone(p.settings), app);
      let own = null;
      try {
        // Plugin UI scripts ship inside the app binary (see mellogram-plugins/ui).
        // They run as async functions so they may `await MG.storage.get()`.
        const AsyncFunction = Object.getPrototypeOf(async function () {}).constructor;
        own = await new AsyncFunction("MG", `"use strict";\n${p.script}\n//# sourceURL=mellogram-plugin:${p.id}.js`)(MG);
      } catch (e) {
        console.error(`plugin ${p.id} failed`, e);
        UI.toast(`UI part failed to start: ${e.message}`, "error", p.id);
      }
      this.running.set(p.id, {
        key,
        dispose: () => { try { typeof own === "function" && own(); } catch (e) { console.error(e); } dispose(); },
      });
    }
  },
};

window.MelloGram = { Bridge, UI, Hooks, PluginHost, h };
