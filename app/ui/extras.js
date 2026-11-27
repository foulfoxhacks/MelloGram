// Command palette, app lock, safe-mode banner and workspace chip.
"use strict";

(() => {
  const { Bridge, UI, h } = window.MelloGram;
  const $ = (id) => document.getElementById(id);
  const App = () => window.MelloGram.App;
  const Settings = () => window.MelloGram.Settings;

  // ---- workspace chip -------------------------------------------------------------

  async function refreshWorkspaceChip() {
    const list = await Bridge.invoke("list_workspaces").catch(() => []);
    const active = list.find((w) => w.active);
    const chip = $("workspace-chip");
    chip.hidden = !list.length;
    chip.textContent = active ? `${active.icon} ${active.name}` : "No workspace";
    return list;
  }
  $("workspace-chip").addEventListener("click", async (e) => {
    e.stopPropagation();
    const list = await refreshWorkspaceChip();
    const r = e.target.getBoundingClientRect();
    UI.contextMenu(r.left, r.bottom + 4, [
      ...list.map((w) => ({ label: `${w.active ? "● " : ""}${w.icon} ${w.name}`, onClick: () => switchWorkspace(w) })),
      { label: "Manage workspaces…", onClick: () => Settings().open("workspaces") },
    ]);
  });

  async function switchWorkspace(w) {
    try {
      await Bridge.invoke("switch_workspace", { id: w.id });
      UI.toast(`Switched to ${w.icon} ${w.name}`, "success");
      await refreshWorkspaceChip();
    } catch (e) {
      UI.toast(String(e), "error");
    }
  }
  Bridge.listen("mg://plugins-changed", refreshWorkspaceChip);

  // ---- command palette --------------------------------------------------------------

  let paletteItems = [];
  let paletteIndex = 0;

  async function buildPaletteItems() {
    const items = [];
    const pages = [
      ["appearance", "Appearance Studio"], ["plugins", "Plugins"], ["themes", "Themes"], ["quickcss", "QuickCSS"],
      ["workspaces", "Workspaces & Sharing"], ["backup", "Backup & Restore"], ["privacy", "Privacy & Security"],
      ["diagnostics", "Diagnostics"], ["telegram", "Telegram account"], ["mellorpc", "MelloRPC Presence"], ["notifications", "Notifications"],
      ["general", "General"], ["about", "About & Updates"],
    ];
    for (const [key, label] of pages) items.push({ group: "Settings", label: `Open ${label}`, run: () => Settings().open(key) });
    items.push({ group: "Actions", label: "Lock MelloGram now", hint: "Ctrl+L", run: () => Lock.lockNow() });
    items.push({ group: "Actions", label: "Run diagnostics tests", run: () => Settings().open("diagnostics") });
    items.push({ group: "Actions", label: "Reload chats", run: () => App().refreshChats() });
    items.push({ group: "Actions", label: "Restart in safe mode", run: () => Bridge.invoke("restart_app", { safeMode: true }) });
    for (const w of await Bridge.invoke("list_workspaces").catch(() => [])) {
      items.push({ group: "Workspaces", label: `Switch to ${w.icon} ${w.name}`, hint: w.active ? "active" : "", run: () => switchWorkspace(w) });
    }
    for (const p of await Bridge.invoke("list_plugins").catch(() => [])) {
      items.push({
        group: "Plugins", label: `${p.enabled ? "Disable" : "Enable"} ${p.name}`, hint: p.category,
        run: async () => {
          try { await Bridge.invoke("set_plugin_enabled", { id: p.id, enabled: !p.enabled }); UI.toast(`${p.name} ${p.enabled ? "disabled" : "enabled"}`, "success"); }
          catch (e) { UI.toast(String(e), "error"); }
        },
      });
    }
    for (const c of App().chats()) items.push({ group: "Chats", label: c.title, hint: c.username ? `@${c.username}` : c.kind, run: () => App().openChat(c) });
    return items;
  }

  function fuzzy(label, q) {
    if (!q) return 1;
    const l = label.toLowerCase();
    if (l.includes(q)) return 100 - l.indexOf(q);
    let i = 0;
    for (const ch of q) { i = l.indexOf(ch, i); if (i < 0) return 0; i++; }
    return 1;
  }

  function renderPalette() {
    const q = $("palette-input").value.trim().toLowerCase();
    const matches = paletteItems.map((it) => [it, fuzzy(`${it.label} ${it.group}`, q)]).filter(([, s]) => s > 0)
      .sort((a, b) => b[1] - a[1]).slice(0, 60).map(([it]) => it);
    paletteIndex = Math.min(paletteIndex, Math.max(matches.length - 1, 0));
    $("palette-list").replaceChildren(...matches.map((it, i) => h("div", {
      class: `mg-palette-item${i === paletteIndex ? " mg-active" : ""}`,
      onmousedown: (e) => { e.preventDefault(); runItem(it); },
    }, h("span", { class: "mg-tag", text: it.group }), h("span", { class: "mg-palette-label", text: it.label }), it.hint ? h("small", { class: "mg-muted", text: it.hint }) : null)));
    $("palette-list").dataset.count = matches.length;
    return matches;
  }

  function runItem(it) {
    closePalette();
    Promise.resolve(it.run()).catch((e) => UI.toast(String(e), "error"));
  }

  async function openPalette() {
    paletteItems = await buildPaletteItems();
    paletteIndex = 0;
    $("palette").hidden = false;
    $("palette-input").value = "";
    renderPalette();
    $("palette-input").focus();
  }
  function closePalette() { $("palette").hidden = true; }

  $("open-palette").addEventListener("click", openPalette);
  $("palette").addEventListener("click", (e) => { if (e.target.id === "palette") closePalette(); });
  $("palette-input").addEventListener("input", () => { paletteIndex = 0; renderPalette(); });
  $("palette-input").addEventListener("keydown", (e) => {
    const matches = renderPalette();
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      paletteIndex = (paletteIndex + (e.key === "ArrowDown" ? 1 : matches.length - 1)) % Math.max(matches.length, 1);
      renderPalette();
    } else if (e.key === "Enter" && matches[paletteIndex]) {
      e.preventDefault();
      runItem(matches[paletteIndex]);
    } else if (e.key === "Escape") {
      closePalette();
    }
  });

  // ---- app lock ------------------------------------------------------------------------

  const Lock = {
    minutes: 0,
    lastInput: Date.now(),
    locked: false,
    async lockNow() {
      const status = await Bridge.invoke("lock_status");
      if (!status.has_pin) {
        UI.toast("Set a PIN first in Settings → Privacy & Security", "info");
        return;
      }
      this.locked = true;
      $("lock").hidden = false;
      $("lock-error").hidden = true;
      $("lock-pin").value = "";
      $("lock-pin").focus();
    },
    configure(minutes) { this.minutes = minutes || 0; },
  };
  window.MelloGram.Lock = Lock;

  for (const ev of ["mousemove", "keydown", "touchstart", "wheel"]) {
    document.addEventListener(ev, () => { if (!Lock.locked) Lock.lastInput = Date.now(); }, { passive: true });
  }
  setInterval(() => {
    if (!Lock.locked && Lock.minutes > 0 && Date.now() - Lock.lastInput > Lock.minutes * 60_000) Lock.lockNow();
  }, 15_000);

  $("lock-form").addEventListener("submit", async (e) => {
    e.preventDefault();
    try {
      if (await Bridge.invoke("verify_lock_pin", { pin: $("lock-pin").value })) {
        Lock.locked = false;
        Lock.lastInput = Date.now();
        $("lock").hidden = true;
      } else {
        $("lock-error").textContent = "Wrong PIN";
        $("lock-error").hidden = false;
      }
    } catch (err) {
      $("lock-error").textContent = String(err);
      $("lock-error").hidden = false;
    }
    $("lock-pin").value = "";
  });

  // ---- shortcuts ------------------------------------------------------------------------

  document.addEventListener("keydown", (e) => {
    const mod = e.ctrlKey || e.metaKey;
    if (mod && !e.shiftKey && e.key.toLowerCase() === "p") { e.preventDefault(); $("palette").hidden ? openPalette() : closePalette(); }
    if (mod && !e.shiftKey && e.key.toLowerCase() === "l") { e.preventDefault(); Lock.lockNow(); }
  });

  // ---- startup -------------------------------------------------------------------------------

  function banner(text, actions) {
    const b = $("banner");
    b.replaceChildren(h("span", { text }), ...actions.map(([label, fn]) => h("button", { class: "mg-btn", text: label, onclick: fn })),
      h("button", { class: "mg-icon-btn", text: "✕", onclick: () => (b.hidden = true) }));
    b.hidden = false;
  }

  window.MelloGram.Extras = {
    async afterBoot(status) {
      if (status.mobile) document.body.classList.add("mg-mobile-app");
      if (status.safe_mode) {
        banner("Safe mode: plugins, themes and custom CSS are off.", [["Restart normally", () => Bridge.invoke("restart_app", { safeMode: false })]]);
      } else if (status.unclean_shutdown) {
        banner("MelloGram didn't close properly last time. If a plugin or theme broke it, try safe mode.", [
          ["Restart in safe mode", () => Bridge.invoke("restart_app", { safeMode: true })],
          ["Run diagnostics", () => Settings().open("diagnostics")],
        ]);
      }
      Lock.configure(status.lock_after_minutes);
      if (status.lock_on_start) Lock.lockNow();
      refreshWorkspaceChip();
    },
    refreshWorkspaceChip,
  };
})();
