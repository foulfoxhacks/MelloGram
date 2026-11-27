// Settings window: Plugins, Themes, QuickCSS, Backup & Restore, Telegram,
// Notifications, General, About and (in developer mode) Developer.
"use strict";

(() => {
  const { Bridge, UI, h } = window.MelloGram;
  const $ = (id) => document.getElementById(id);

  const CATEGORY_LABELS = {
    appearance: "Appearance", chat: "Chat", media: "Media", privacy: "Privacy", profiles: "Chats & Profiles",
    notifications: "Notifications", developer: "Developer", utility: "Utility",
    productivity: "Productivity", accessibility: "Accessibility",
  };
  const PERMISSION_LABELS = {
    "read-messages": "reads messages", "modify-outgoing": "changes what you send", ui: "adds UI",
    storage: "stores data", network: "network", clipboard: "clipboard", styles: "styles",
  };
  const P = () => window.MelloGram.Pages;

  let current = "plugins";
  let settings = null;

  const pages = {
    appearance: { group: "Customize", label: "Appearance Studio", render: () => P().appearance() },
    wallpapers: { group: "Customize", label: "Wallpapers", render: () => P().wallpapers() },
    plugins: { group: "Customize", label: "Plugins", render: renderPlugins },
    themes: { group: "Customize", label: "Themes", render: renderThemes },
    quickcss: { group: "Customize", label: "QuickCSS", render: renderQuickCss },
    workspaces: { group: "Customize", label: "Workspaces & Sharing", render: () => P().workspaces() },
    backup: { group: "Data", label: "Backup & Restore", render: renderBackup },
    privacy: { group: "Security", label: "Privacy & Security", render: () => P().privacy() },
    e2e: { group: "Security", label: "End-to-End Overlay", render: () => P().e2e() },
    diagnostics: { group: "System", label: "Diagnostics", render: () => P().diagnostics() },
    telegram: { group: "System", label: "Telegram", render: renderTelegram },
    mellorpc: { group: "System", label: "MelloRPC Presence", render: renderMelloRpc },
    notifications: { group: "System", label: "Notifications", render: renderNotifications },
    media: { group: "System", label: "Media & Storage", render: renderMedia },
    general: { group: "System", label: "General", render: renderGeneral },
    about: { group: "System", label: "About & Updates", render: renderAbout },
    developer: { group: "System", label: "Developer", render: renderDeveloper, devOnly: true },
  };

  async function open(page) {
    settings = await Bridge.invoke("get_settings");
    if (page) current = page;
    $("settings").hidden = false;
    renderNav();
    await show(current);
  }

  function close() { $("settings").hidden = true; }

  function renderNav() {
    const nav = [];
    let group = null;
    for (const [key, p] of Object.entries(pages)) {
      if (p.devOnly && !settings.general.developer_mode) continue;
      if (p.group !== group) { group = p.group; nav.push(h("h4", { text: group })); }
      nav.push(h("button", { class: key === current ? "mg-active" : "", text: p.label, onclick: () => show(key) }));
    }
    $("settings-nav").replaceChildren(...nav);
  }

  async function show(key) {
    current = key;
    renderNav();
    const page = $("settings-page");
    page.replaceChildren(h("p", { class: "mg-muted", text: "Loading…" }));
    try {
      page.replaceChildren(...[await pages[key].render()].flat().filter((n) => n != null && n !== false));
    } catch (e) {
      page.replaceChildren(h("p", { class: "mg-error", text: String(e) }));
    }
  }

  $("open-settings").addEventListener("click", () => open());
  $("close-settings").addEventListener("click", close);
  $("settings").addEventListener("click", (e) => { if (e.target.id === "settings") close(); });
  document.addEventListener("keydown", (e) => { if (e.key === "Escape" && !$("settings").hidden && !document.querySelector(".mg-modal")) close(); });

  const call = async (cmd, args, ok) => {
    try { const r = await Bridge.invoke(cmd, args); if (ok) UI.toast(ok, "success"); return r; }
    catch (e) { UI.toast(String(e), "error"); throw e; }
  };

  function sw(checked, onchange, disabled = false) {
    return h("input", { type: "checkbox", class: "mg-switch", checked, disabled, onchange: (e) => onchange(e.target.checked, e.target) });
  }

  function download(name, text) {
    const url = URL.createObjectURL(new Blob([text], { type: "application/json" }));
    h("a", { href: url, download: name }).click();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  }

  function pickFile(accept) {
    return new Promise((resolve) => {
      const input = h("input", { type: "file", accept });
      input.onchange = () => resolve(input.files[0] || null);
      input.click();
    });
  }

  // ---- Plugins ----------------------------------------------------------------

  const pluginFilter = { q: "", category: "all", status: "all" };

  async function renderPlugins() {
    const plugins = await Bridge.invoke("list_plugins");
    const grid = h("div", { class: "mg-grid" });
    const counter = h("span", { class: "mg-muted" });

    const draw = () => {
      const q = pluginFilter.q.toLowerCase();
      const list = plugins
        .filter((p) => !q || `${p.name} ${p.description} ${p.id} ${p.inspired_by || ""}`.toLowerCase().includes(q))
        .filter((p) => pluginFilter.category === "all" || p.category === pluginFilter.category)
        .filter((p) => ({ all: true, enabled: p.enabled, disabled: !p.enabled, favorites: p.favorite, errors: p.errors.length })[pluginFilter.status])
        .sort((a, b) => (b.favorite - a.favorite) || a.name.localeCompare(b.name));
      counter.textContent = `${plugins.filter((p) => p.enabled).length} of ${plugins.length} enabled`;
      grid.replaceChildren(...list.map(pluginCard));
      if (!list.length) grid.append(h("p", { class: "mg-muted", text: "No plugins match." }));
    };

    function pluginCard(p) {
      const card = h("div", { class: `mg-plugin-card${p.errors.length ? " mg-has-error" : ""}` },
        h("div", { class: "mg-plugin-head" },
          h("button", { class: "mg-icon-btn", title: p.favorite ? "Unfavourite" : "Favourite", text: p.favorite ? "★" : "☆",
            onclick: async () => { p.favorite = !p.favorite; await call("set_plugin_favorite", { id: p.id, favorite: p.favorite }); draw(); } }),
          h("b", { text: p.name }),
          p.settings_schema.length ? h("button", { class: "mg-icon-btn", title: "Settings", text: "⚙", onclick: () => pluginSettings(p) }) : null,
          sw(p.enabled, async (on, el) => {
            try {
              const changed = await call("set_plugin_enabled", { id: p.id, enabled: on });
              for (const id of changed) { const q = plugins.find((x) => x.id === id); if (q) q.enabled = on; }
              if (on && changed.length > 1) UI.toast(`Also enabled: ${changed.filter((x) => x !== p.id).join(", ")}`, "info");
              draw();
            } catch { el.checked = !on; }
          }, !p.supported || p.required)),
        h("div", { class: "mg-plugin-desc", text: p.description }),
        h("div", { class: "mg-plugin-tags" },
          h("span", { class: "mg-tag", text: CATEGORY_LABELS[p.category] || p.category }),
          h("span", { class: "mg-tag", text: `v${p.version}` }),
          p.has_ui ? h("span", { class: "mg-tag", text: "UI" }) : null,
          p.inspired_by ? h("span", { class: "mg-tag", title: "Feature this adapts", text: p.inspired_by }) : null,
          p.dependencies.length ? h("span", { class: "mg-tag", text: `needs ${p.dependencies.join(", ")}` }) : null,
          ...p.permissions.map((perm) => h("span", { class: `mg-tag mg-perm mg-perm-${perm}`, title: "Permission", text: PERMISSION_LABELS[perm] || perm })),
          p.enabled && p.stats.calls ? h("span", { class: "mg-tag", title: "Hook calls this session / average time", text: `${p.stats.calls} calls · ${Math.round(p.stats.total_us / p.stats.calls)}µs` }) : null,
          !p.supported ? h("span", { class: "mg-tag", text: "unsupported here" }) : null),
        p.errors.length ? h("div", { class: "mg-error mg-hint", text: p.errors.at(-1) }) : null);
      return card;
    }

    function pluginSettings(p) {
      const form = h("div");
      for (const def of p.settings_schema) form.append(settingField(p, def));
      UI.modal(`${p.name} settings`, [
        h("p", { class: "mg-muted", text: `${p.description} — by ${p.authors.join(", ")}` }),
        form,
        h("div", { class: "mg-toolbar" }, h("button", {
          class: "mg-btn mg-danger", text: "Reset to defaults",
          onclick: async () => { await call("reset_plugin_settings", { id: p.id }, "Settings reset"); document.querySelector(".mg-modal").closest(".mg-overlay").remove(); show("plugins"); },
        })),
      ]);
    }

    function settingField(p, def) {
      const value = p.settings[def.key];
      const save = (v) => call("set_plugin_setting", { id: p.id, key: def.key, value: v }).then(() => { p.settings[def.key] = v; });
      const hint = def.description ? h("div", { class: "mg-hint", text: def.description }) : null;
      switch (def.type) {
        case "bool":
          return h("div", { class: "mg-field-row" }, h("div", {}, h("label", { text: def.label }), hint), sw(!!value, (on) => save(on)));
        case "number": {
          const input = h("input", { type: "number", value, min: def.min, max: def.max, onchange: (e) => save(Number(e.target.value)) });
          return h("div", { class: "mg-field" }, h("label", { text: def.label }), hint, input);
        }
        case "select":
          return h("div", { class: "mg-field" }, h("label", { text: def.label }), hint,
            h("select", { onchange: (e) => save(e.target.value) }, ...def.options.map(([v, l]) => h("option", { value: v, selected: v === value, text: l }))));
        case "list": {
          const ta = h("textarea", { rows: 5, spellcheck: false, onchange: (e) => save(e.target.value.split("\n").map((s) => s.trim()).filter(Boolean)) });
          ta.value = (value || []).join("\n");
          return h("div", { class: "mg-field" }, h("label", { text: def.label }), hint, ta);
        }
        default: {
          const input = h(def.multiline ? "textarea" : "input", { type: "text", onchange: (e) => save(e.target.value) });
          input.value = value ?? "";
          return h("div", { class: "mg-field" }, h("label", { text: def.label }), hint, input);
        }
      }
    }

    const categories = [...new Set(plugins.map((p) => p.category))].sort();
    const search = h("input", { type: "search", placeholder: "Search plugins…", value: pluginFilter.q, oninput: (e) => { pluginFilter.q = e.target.value; draw(); } });
    const toolbar = h("div", { class: "mg-toolbar" },
      search,
      h("select", { onchange: (e) => { pluginFilter.category = e.target.value; draw(); } },
        h("option", { value: "all", text: "All categories" }),
        ...categories.map((c) => h("option", { value: c, selected: c === pluginFilter.category, text: CATEGORY_LABELS[c] || c }))),
      h("select", { onchange: (e) => { pluginFilter.status = e.target.value; draw(); } },
        ...["all", "enabled", "disabled", "favorites", "errors"].map((s) => h("option", { value: s, selected: s === pluginFilter.status, text: s[0].toUpperCase() + s.slice(1) }))));
    draw();
    setTimeout(() => search.focus());
    return [h("h2", { text: "Plugins" }), counter, toolbar, grid];
  }

  // ---- Themes -----------------------------------------------------------------

  async function renderThemes() {
    const themes = await Bridge.invoke("list_themes");
    let enabled = [...settings.themes.enabled];
    const info = await Bridge.invoke("app_info");

    const saveOrder = async () => {
      await call("set_enabled_themes", { files: enabled });
      settings.themes.enabled = enabled;
      show("themes");
    };
    const move = (file, delta) => {
      const i = enabled.indexOf(file);
      const j = i + delta;
      if (i < 0 || j < 0 || j >= enabled.length) return;
      [enabled[i], enabled[j]] = [enabled[j], enabled[i]];
      saveOrder();
    };

    const ordered = [...enabled.map((f) => themes.find((t) => t.file === f)).filter(Boolean), ...themes.filter((t) => !enabled.includes(t.file))];
    const rows = ordered.map((t) => {
      const on = enabled.includes(t.file);
      const overrides = settings.themes.variables[t.file] || {};
      const vars = t.variables.length && on ? h("details", {}, h("summary", { class: "mg-hint", text: `${t.variables.length} variables` }),
        h("div", { class: "mg-vars" }, ...t.variables.flatMap(([name, def]) => {
          const val = overrides[name] ?? def;
          const text = h("input", { type: "text", value: val, onchange: (e) => setVar(t.file, name, e.target.value || null) });
          const isHex = /^#[0-9a-f]{6}$/i.test(val);
          return [h("code", { text: name }), text,
            isHex ? h("input", { type: "color", value: val, onchange: (e) => setVar(t.file, name, e.target.value) }) : h("span")];
        }))) : null;
      return h("div", {},
        h("div", { class: "mg-theme-row" },
          sw(on, (v) => { enabled = v ? [...enabled, t.file] : enabled.filter((f) => f !== t.file); saveOrder(); }),
          h("div", { class: "mg-theme-info" },
            h("b", { text: t.meta.name || t.file }),
            h("div", { class: "mg-hint", text: [t.meta.author && `by ${t.meta.author}`, t.meta.version && `v${t.meta.version}`, t.meta.description].filter(Boolean).join(" · ") })),
          on ? h("button", { class: "mg-icon-btn", title: "Apply earlier", text: "↑", onclick: () => move(t.file, -1) }) : null,
          on ? h("button", { class: "mg-icon-btn", title: "Apply later (wins)", text: "↓", onclick: () => move(t.file, 1) }) : null,
          h("button", { class: "mg-icon-btn", title: "View / edit source", text: "✎", onclick: () => editTheme(t) }),
          h("button", { class: "mg-icon-btn", title: "Delete", text: "🗑", onclick: async () => {
            if (await UI.confirm("Delete theme?", `Remove ${t.meta.name || t.file} from your themes folder?`, "Delete")) {
              await call("remove_theme", { file: t.file }, "Theme removed"); settings = await Bridge.invoke("get_settings"); show("themes");
            }
          } })),
        vars);
    });

    async function setVar(file, name, value) {
      await call("set_theme_variable", { file, name, value });
      settings = await Bridge.invoke("get_settings");
    }

    async function editTheme(t) {
      const ta = h("textarea", { class: "mg-code", spellcheck: false });
      ta.value = await Bridge.invoke("get_theme_source", { file: t.file });
      const close = UI.modal(t.meta.name || t.file, [ta, h("div", { class: "mg-toolbar" },
        h("button", { class: "mg-btn mg-primary", text: "Save", onclick: async () => { await call("install_theme", { name: t.file, css: ta.value }, "Theme saved"); close(); show("themes"); } }))]);
    }

    const urlInput = h("input", { type: "text", placeholder: "https://example.com/my-theme.theme.css" });
    return [
      h("h2", { text: "Themes" }),
      h("p", { class: "mg-muted", text: "Enabled themes apply top to bottom; later ones win. QuickCSS always applies last." }),
      h("div", { class: "mg-toolbar" },
        h("button", { class: "mg-btn", text: "Import .css file", onclick: async () => {
          const f = await pickFile(".css,text/css");
          if (f) { await call("install_theme", { name: f.name, css: await f.text() }, "Theme installed"); show("themes"); }
        } }),
        h("button", { class: "mg-btn", text: "New theme", onclick: async () => {
          await call("install_theme", { name: "My Theme", css: "/**\n * @name My Theme\n * @author me\n * @version 1.0.0\n */\n:root {\n  --mg-accent: #ff8fab;\n}\n" });
          show("themes");
        } })),
      h("div", { class: "mg-toolbar" }, urlInput,
        h("button", { class: "mg-btn", text: "Install from URL", onclick: async () => { await call("install_theme_url", { url: urlInput.value.trim() }, "Theme installed"); show("themes"); } })),
      ...rows,
      themes.length ? null : h("p", { class: "mg-muted", text: "No themes yet." }),
      h("p", { class: "mg-hint", text: `Themes folder: ${info.themes_dir}` }),
    ];
  }

  // ---- QuickCSS ------------------------------------------------------------------

  function renderQuickCss() {
    const ta = h("textarea", { class: "mg-code", spellcheck: false, placeholder: "/* Your CSS here. Try: */\n:root { --mg-accent: #ff8fab; }" });
    ta.value = settings.themes.quick_css;
    let enabled = settings.themes.quick_css_enabled;
    let timer;
    const status = h("span", { class: "mg-hint" });
    const save = () => {
      clearTimeout(timer);
      status.textContent = "…";
      timer = setTimeout(async () => {
        await call("set_quick_css", { css: ta.value, enabled });
        settings.themes.quick_css = ta.value;
        status.textContent = "Applied";
      }, 300);
    };
    ta.addEventListener("input", save);
    ta.addEventListener("keydown", (e) => {
      if (e.key === "Tab") { e.preventDefault(); ta.setRangeText("  ", ta.selectionStart, ta.selectionEnd, "end"); save(); }
    });
    return [
      h("h2", { text: "QuickCSS" }),
      h("div", { class: "mg-field-row" }, h("div", {}, h("label", { text: "Enable QuickCSS" }), h("div", { class: "mg-hint", text: "Changes apply live as you type." })),
        sw(enabled, (v) => { enabled = v; save(); })),
      ta, status,
      h("p", { class: "mg-hint", text: "Every element has an mg- class (mg-message, mg-chat-item, mg-composer…) and colours are --mg-* variables. See docs/THEMING.md." }),
    ];
  }

  // ---- Backup -----------------------------------------------------------------------

  function renderBackup() {
    return [
      h("h2", { text: "Backup & Restore" }),
      h("p", { class: "mg-muted", text: "Exports plugins, plugin settings, themes, QuickCSS and preferences to one JSON file. Your Telegram login and API credentials are never included." }),
      h("div", { class: "mg-toolbar" },
        h("button", { class: "mg-btn mg-primary", text: "Export settings", onclick: async () => {
          const json = await call("export_backup");
          download(`mellogram-backup-${new Date().toISOString().slice(0, 10)}.json`, json);
        } }),
        h("button", { class: "mg-btn", text: "Import settings", onclick: async () => {
          const f = await pickFile(".json,application/json");
          if (!f) return;
          if (!(await UI.confirm("Import backup?", "This replaces your current MelloGram settings.", "Import"))) return;
          const n = await call("import_backup", { json: await f.text() });
          UI.toast(`Backup restored (${n} theme${n === 1 ? "" : "s"})`, "success");
          settings = await Bridge.invoke("get_settings");
        } })),
    ];
  }

  // ---- Telegram ------------------------------------------------------------------

  async function renderTelegram() {
    const login = await Bridge.invoke("login_state").catch(() => null);
    const id = h("input", { type: "number", value: settings.telegram.api_id ?? "" });
    const hash = h("input", { type: "password", placeholder: settings.telegram.has_hash ? "•••••••• (saved)" : "32 hex characters" });
    return [
      h("h2", { text: "Telegram" }),
      h("p", { class: "mg-muted", text: login?.state === "ready" ? `Signed in as ${login.me.name}${login.me.username ? ` (@${login.me.username})` : ""}.` : "Not signed in." }),
      h("div", { class: "mg-field" }, h("label", { text: "API ID" }), id),
      h("div", { class: "mg-field" }, h("label", { text: "API hash" }), hash,
        h("div", { class: "mg-hint", text: "From my.telegram.org → API development tools. Stored only in this device's settings file." })),
      h("div", { class: "mg-toolbar" },
        h("button", { class: "mg-btn mg-primary", text: "Save & reconnect", onclick: () => call("set_credentials", { apiId: Number(id.value), apiHash: hash.value }, "Saved") }),
        login?.state === "ready" ? h("button", { class: "mg-btn mg-danger", text: "Log out", onclick: async () => {
          if (await UI.confirm("Log out?", "This ends the MelloGram session on Telegram's side.", "Log out")) { await call("logout"); location.reload(); }
        } }) : null),
    ];
  }

  // ---- Notifications / General ------------------------------------------------------

  function renderNotifications() {
    const n = { ...settings.notifications };
    const save = async () => { await call("set_notifications", { notifications: n }); settings.notifications = n; $("toasts").className = `mg-toasts ${n.position}`; window.MelloGram.UI.toastTimeout = n.timeout_ms; };
    return [
      h("h2", { text: "Notifications" }),
      h("div", { class: "mg-field" }, h("label", { text: "Toast position" }),
        h("select", { onchange: (e) => { n.position = e.target.value; save(); } },
          ...["top-left", "top-right", "bottom-left", "bottom-right"].map((p) => h("option", { value: p, selected: p === n.position, text: p.replace("-", " ") })))),
      h("div", { class: "mg-field" }, h("label", { text: "Toast duration (ms)" }),
        h("input", { type: "number", min: 1000, max: 60000, value: n.timeout_ms, onchange: (e) => { n.timeout_ms = Number(e.target.value); save(); } })),
      h("div", { class: "mg-field-row" }, h("label", { text: "Test" }), h("button", { class: "mg-btn", text: "Show a toast", onclick: () => UI.toast("Hello from MelloGram!", "success") })),
      h("h3", { text: "Message notifications" }),
      h("p", { class: "mg-hint", text: "Native notifications appear while MelloGram is hidden, minimised to the tray, or in the background." }),
      nrow("desktop", "Notify about new messages", "Uses your system's notification centre."),
      nrow("show_previews", "Show sender and message text", "Turn off to show only “New message” — useful on shared screens."),
      nrow("dnd", "Do Not Disturb", "No message notifications at all. Also in the tray menu."),
      nrow("notify_own", "Notify for my messages from other devices", ""),
      nrow("quiet_hours", "Quiet hours", "Silence notifications every day between these hours."),
      h("div", { class: "mg-toolbar" },
        h("label", { text: "From" }), h("input", { type: "number", min: 0, max: 23, value: n.quiet_start, style: "width:80px", onchange: (e) => { n.quiet_start = Math.min(23, Math.max(0, Number(e.target.value) || 0)); save(); } }),
        h("label", { text: "to" }), h("input", { type: "number", min: 0, max: 23, value: n.quiet_end, style: "width:80px", onchange: (e) => { n.quiet_end = Math.min(23, Math.max(0, Number(e.target.value) || 0)); save(); } }),
        h("span", { class: "mg-hint", text: "(24-hour clock, local time)" })),
      h("p", { class: "mg-hint", text: `${(n.muted_chats || []).length} muted chat(s). Right-click a chat → Mute notifications.` }),
    ];
    function nrow(key, label, hint) {
      return h("div", { class: "mg-field-row" }, h("div", {}, h("label", { text: label }), hint ? h("div", { class: "mg-hint", text: hint }) : null), sw(!!n[key], (v) => { n[key] = v; save(); }));
    }
  }

  async function renderMedia() {
    const m = await Bridge.invoke("get_media_settings");
    const save = async () => { await call("set_media_settings", { media: m }); await window.MelloGram.Media?.reloadSettings(); };
    const toggle = (key, label, hint) => h("div", { class: "mg-field-row" }, h("div", {}, h("label", { text: label }), hint ? h("div", { class: "mg-hint", text: hint }) : null), sw(!!m[key], (v) => { m[key] = v; save(); }));
    return [
      h("h2", { text: "Media & Storage" }),
      h("div", { class: "mg-field" }, h("label", { text: "Download photos, stickers and GIFs automatically up to (MB)" }),
        h("input", { type: "number", min: 0, max: 200, value: m.auto_download_mb, onchange: (e) => { m.auto_download_mb = Math.max(0, Math.min(200, Number(e.target.value) || 0)); save(); } }),
        h("div", { class: "mg-hint", text: "0 = only download when you click. Videos and files always wait for a click." })),
      h("div", { class: "mg-field" }, h("label", { text: "Largest file you can download (MB)" }),
        h("input", { type: "number", min: 1, max: 4000, value: m.max_download_mb, onchange: (e) => { m.max_download_mb = Math.max(1, Math.min(4000, Number(e.target.value) || 1)); save(); } })),
      toggle("autoplay_gifs", "Autoplay GIFs", "Silent and looping, like Telegram."),
      toggle("autoplay_video_stickers", "Autoplay video stickers", ""),
      toggle("respect_spoilers", "Blur media marked as spoiler", "Click to reveal."),
      toggle("load_map_tiles", "Show map previews for locations", "Loads map images from OpenStreetMap, which then learns roughly which places you look at. Off: coordinates and an “Open map” link only."),
      h("div", { class: "mg-field-row" }, h("div", {}, h("label", { text: "Media cache" }), h("div", { class: "mg-hint", text: "Downloaded media is kept on this device so it opens instantly next time." })),
        h("button", { class: "mg-btn", text: "Clear cache", onclick: async () => { const freed = await call("clear_media_cache"); UI.toast(`Freed ${(freed / 1048576).toFixed(1)} MB`, "success"); } })),
    ];
  }

  function renderGeneral() {
    const g = { ...settings.general };
    const save = async () => { await call("set_general", { general: g }); settings.general = g; renderNav(); };
    const row = (key, label, hint) => h("div", { class: "mg-field-row" }, h("div", {}, h("label", { text: label }), h("div", { class: "mg-hint", text: hint })), sw(g[key], (v) => { g[key] = v; save(); }));
    return [
      h("h2", { text: "General" }),
      row("auto_check_updates", "Check for updates on launch", "Looks for new GitHub releases. Nothing else is sent."),
      row("developer_mode", "Developer mode", "Shows the Developer tab with the live stylesheet and event log."),
      h("h3", { text: "Background" }),
      row("close_to_tray", "Keep running in the tray when closed", "Closing the window hides MelloGram; it keeps receiving messages and notifying you. Quit from the tray icon."),
      row("start_minimized", "Start minimised to the tray", "Opens in the background; click the tray icon to show it."),
      row("launch_at_login", "Launch when you sign in", "Starts MelloGram (minimised) when you log in to your computer."),
      h("p", { class: "mg-hint", text: "Tray, background and launch-at-login are available on Windows, Linux and macOS." }),
    ];
  }


  // ---- MelloRPC ------------------------------------------------------------------

  async function renderMelloRpc() {
    const rpc = { ...(settings.rpc || { native_enabled: false, discord_compat_enabled: false }) };
    const row = (key, title, hint) => h("div", { class: "mg-field-row" },
      h("div", {}, h("label", { text: title }), h("div", { class: "mg-hint", text: hint })),
      sw(rpc[key], (value) => { rpc[key] = value; }));
    const save = async (restart) => {
      await call("set_rpc_settings", { rpc }, "MelloRPC settings saved.");
      settings.rpc = { ...rpc };
      if (restart) await Bridge.invoke("restart_app", { safeMode: false });
      else UI.toast("Restart MelloGram to apply the local IPC listeners.", "info");
    };
    const endpoint = navigator.userAgent.includes("Windows")
      ? "\\\\.\\pipe\\mellorpc-v1"
      : "mellorpc.sock in the MelloGram config directory";
    return [
      h("h2", { text: "MelloRPC Presence" }),
      h("p", { class: "mg-muted", text: "Local rich presence for games and apps. These listeners are disabled by default and never open a TCP port." }),
      row("native_enabled", "Enable native MelloRPC", "Lets applications send structured activity to MelloGram through a local IPC endpoint."),
      row("discord_compat_enabled", "Enable Discord Rich Presence compatibility", "Accepts the supported Discord IPC handshake and SET_ACTIVITY / CLEAR_ACTIVITY commands. Not every Discord RPC command is implemented."),
      h("p", { class: "mg-hint", text: `Native endpoint: ${endpoint}. Discord compatibility uses the local discord-ipc-N socket or named pipe discovered by Discord SDKs.` }),
      h("p", { class: "mg-hint", text: "When a separate Discord desktop client already owns the standard IPC endpoint, Discord SDKs may continue connecting to that client instead of MelloGram. Close it or configure the SDK to use MelloGram's available endpoint for compatibility testing." }),
      h("div", { class: "mg-toolbar" },
        h("button", { class: "mg-btn", text: "Save settings", onclick: () => save(false) }),
        h("button", { class: "mg-btn mg-primary", text: "Save & Restart", onclick: () => save(true) })),
    ];
  }

  // ---- About ------------------------------------------------------------------------

  async function renderAbout() {
    const info = await Bridge.invoke("app_info");
    const changelog = h("pre", { class: "mg-raw", text: info.changelog });
    return [
      h("h2", { text: `MelloGram ${info.version}` }),
      h("p", { class: "mg-muted", text: `${info.platform} / ${info.arch} · config: ${info.config_dir}` }),
      h("p", { class: "mg-muted", text: "MelloGram is an unofficial Telegram client. It is not affiliated with Telegram." }),
      h("h3", { text: "Changelog" }), changelog,
    ];
  }

  // ---- Developer ------------------------------------------------------------------

  const eventLog = [];
  Bridge.listen("mg://event", (e) => { eventLog.push({ at: new Date().toISOString(), ...e }); if (eventLog.length > 200) eventLog.shift(); });

  async function renderDeveloper() {
    const css = await Bridge.invoke("get_stylesheet");
    return [
      h("h2", { text: "Developer" }),
      h("h3", { text: "Composed stylesheet" }), h("pre", { class: "mg-raw", text: css || "/* empty */" }),
      h("h3", { text: `Recent events (${eventLog.length})` }), h("pre", { class: "mg-raw", text: JSON.stringify(eventLog.slice(-50), null, 2) }),
      h("div", { class: "mg-toolbar" }, h("button", { class: "mg-btn", text: "Reload UI", onclick: () => location.reload() })),
    ];
  }

  window.MelloGram.Settings = { open, close, show: (k) => show(k), reloadSettings: async () => { settings = await Bridge.invoke("get_settings"); return settings; } };
  window.MelloGram.SettingsKit = { sw, call, download, pickFile, CATEGORY_LABELS, PERMISSION_LABELS };
})();
