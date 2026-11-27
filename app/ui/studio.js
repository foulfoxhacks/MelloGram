// Settings pages: Appearance Studio, Workspaces & Sharing, Privacy & Security,
// Diagnostics. Registered into the settings window via window.MelloGram.Pages.
"use strict";

(() => {
  const { Bridge, UI, h } = window.MelloGram;
  const Kit = () => window.MelloGram.SettingsKit;
  const App = () => window.MelloGram.App;

  // ---- shared controls ----------------------------------------------------------------

  /** Cloudflare-style expandable configuration section. */
  function section(title, subtitle, open, ...body) {
    return h("details", { class: "mg-section", open },
      h("summary", {}, h("div", {}, h("b", { text: title }), h("div", { class: "mg-hint", text: subtitle })), h("span", { class: "mg-chevron", text: "›" })),
      h("div", { class: "mg-section-body" }, ...body));
  }

  function row(label, control, hint) {
    return h("div", { class: "mg-control" }, h("div", {}, h("label", { text: label }), hint ? h("div", { class: "mg-hint", text: hint }) : null), control);
  }

  function slider(obj, key, min, max, unit, save, step = 1) {
    const out = h("output", { text: `${obj[key]}${unit}` });
    const input = h("input", { type: "range", min, max, step, value: obj[key], oninput: (e) => {
      obj[key] = Number(e.target.value);
      out.textContent = `${obj[key]}${unit}`;
      save();
    } });
    return h("div", { class: "mg-slider" }, input, out);
  }

  function segmented(obj, key, options, save) {
    const wrap = h("div", { class: "mg-segmented" });
    const draw = () => wrap.replaceChildren(...options.map(([v, l]) => h("button", {
      class: obj[key] === v ? "mg-active" : "", text: l, type: "button",
      onclick: () => { obj[key] = v; draw(); save(); },
    })));
    draw();
    return wrap;
  }

  function select(obj, key, options, save) {
    return h("select", { onchange: (e) => { obj[key] = e.target.value; save(); } },
      ...options.map(([v, l]) => h("option", { value: v, selected: obj[key] === v, text: l })));
  }

  const toHex = (c) => {
    const ctx = document.createElement("canvas").getContext("2d");
    ctx.fillStyle = "#000";
    ctx.fillStyle = c || "#000";
    return /^#[0-9a-f]{6}$/i.test(ctx.fillStyle) ? ctx.fillStyle : "#000000";
  };

  function color(obj, key, save, placeholder = "theme default") {
    const text = h("input", { type: "text", value: obj[key] || "", placeholder, oninput: (e) => { obj[key] = e.target.value; picker.value = toHex(obj[key]); save(); } });
    const picker = h("input", { type: "color", value: toHex(obj[key]), oninput: (e) => { obj[key] = e.target.value; text.value = e.target.value; save(); } });
    const clear = h("button", { class: "mg-icon-btn", type: "button", title: "Use theme default", text: "↺", onclick: () => { obj[key] = ""; text.value = ""; save(); } });
    return h("div", { class: "mg-color" }, picker, text, clear);
  }

  function fill(obj, save) {
    const preview = h("div", { class: "mg-fill-preview" });
    const update = () => {
      preview.style.background = obj.enabled && obj.from ? (obj.to ? `linear-gradient(${obj.angle}deg, ${obj.from}, ${obj.to})` : obj.from) : "transparent";
      save();
    };
    const sw = Kit().sw(obj.enabled, (v) => { obj.enabled = v; update(); });
    const box = h("div", { class: "mg-fill" },
      h("div", { class: "mg-control" }, h("label", { text: "Enabled" }), sw),
      row("From", color(obj, "from", update, "#8b7cf6")),
      row("To", color(obj, "to", update, "leave empty for a solid colour")),
      row("Angle", slider(obj, "angle", 0, 359, "°", update)),
      preview);
    update();
    return box;
  }

  function debounce(fn, ms) {
    let t;
    return (...args) => { clearTimeout(t); t = setTimeout(() => fn(...args), ms); };
  }

  // ---- Appearance Studio -----------------------------------------------------------------

  async function appearancePage() {
    const a = await Bridge.invoke("get_appearance");
    const status = h("span", { class: "mg-hint" });
    const save = debounce(async () => {
      try { await Bridge.invoke("set_appearance", { appearance: a }); status.textContent = "Saved · applied live"; }
      catch (e) { status.textContent = String(e); }
    }, 150);

    const preview = h("div", { class: "mg-preview mg-chat" },
      h("div", { class: "mg-messages" },
        h("div", { class: "mg-day", text: "Today" }),
        h("div", { class: "mg-message" }, h("div", { class: "mg-sender", text: "Ava" }), h("div", { class: "mg-text", text: "Does the new bubble style look good?" }),
          h("div", { class: "mg-meta" }, h("span", { class: "mg-time", text: "14:02" }))),
        h("div", { class: "mg-message mg-out" }, h("div", { class: "mg-reply-preview", text: "Does the new bubble style look good?" }), h("div", { class: "mg-text", text: "Love it — try the gradient too ✨" }),
          h("div", { class: "mg-meta" }, h("span", { class: "mg-time", text: "14:03" }))),
        h("div", { class: "mg-message" }, h("div", { class: "mg-sender", text: "Ava" }), h("div", { class: "mg-text" }, "Code too: ", h("code", { text: "--mg-radius" })),
          h("div", { class: "mg-meta" }, h("span", { class: "mg-time", text: "14:04" })))));

    const chats = App().chats();
    const chatPick = h("select", {}, h("option", { value: "", text: "Choose a chat…" }), ...chats.map((c) => h("option", { value: String(c.id), text: c.title })));
    const perChatList = h("div");
    const drawPerChat = () => {
      perChatList.replaceChildren(...Object.entries(a.per_chat).map(([id, cfg]) => {
        const title = chats.find((c) => String(c.id) === id)?.title || id;
        return section(title, "Wallpaper, bubbles and accent for this chat", false,
          row("Accent", color(cfg, "accent", save)),
          row("Incoming bubbles", color(cfg, "bubble_in", save)),
          row("Outgoing bubbles", color(cfg, "bubble_out", save)),
          h("h4", { text: "Wallpaper" }), fill(cfg.background, save),
          h("button", { class: "mg-btn mg-danger", type: "button", text: "Remove override", onclick: () => { delete a.per_chat[id]; drawPerChat(); save(); } }));
      }));
    };
    drawPerChat();

    const fonts = [["", "Theme default"], ["Inter, system-ui, sans-serif", "Inter"], ["'Segoe UI', system-ui, sans-serif", "Segoe UI"], ["Ubuntu, system-ui, sans-serif", "Ubuntu"],
      ["Georgia, serif", "Georgia (serif)"], ["'Atkinson Hyperlegible', system-ui, sans-serif", "Atkinson Hyperlegible"], ["OpenDyslexic, 'Comic Sans MS', sans-serif", "OpenDyslexic"], ["ui-monospace, monospace", "Monospace"]];
    const fontSelect = select(a, "font_family", fonts.some(([v]) => v === a.font_family) ? fonts : [...fonts, [a.font_family, a.font_family]], save);

    return [
      h("h2", { text: "Appearance Studio" }),
      h("p", { class: "mg-muted", text: "Visual settings without CSS. Applied after your themes and before QuickCSS, so themes still work and QuickCSS can still override anything." }),
      h("div", { class: "mg-control" }, h("div", {}, h("label", { text: "Use Appearance Studio" }), status), Kit().sw(a.enabled, (v) => { a.enabled = v; save(); })),
      h("div", { class: "mg-studio" },
        h("div", { class: "mg-studio-controls" },
          section("Message appearance", "Bubble style, radius, spacing, animations", true,
            row("Bubble style", segmented(a, "bubble_style", [["rounded", "Rounded"], ["squared", "Squared"], ["compact", "Compact"], ["glass", "Glass"], ["transparent", "Outline"], ["flat", "Flat"]], save)),
            row("Bubble radius", slider(a, "bubble_radius", 0, 32, " px", save)),
            row("Message spacing", slider(a, "message_spacing", 0, 24, " px", save)),
            row("Maximum bubble width", slider(a, "max_bubble_width", 40, 100, "%", save)),
            row("Timestamps", segmented(a, "timestamps", [["inline", "Inline"], ["hover", "On hover"], ["hidden", "Hidden"]], save)),
            row("Reply quotes", segmented(a, "quote_style", [["bar", "Bar"], ["box", "Box"], ["minimal", "Minimal"]], save)),
            row("Show sender names", Kit().sw(a.show_sender_names, (v) => { a.show_sender_names = v; save(); })),
            row("New message animation", segmented(a, "message_animation", [["none", "None"], ["fade", "Fade"], ["slide", "Slide"], ["pop", "Pop"]], save))),
          section("Colours & gradients", "Accent, bubble fills and chat wallpaper", false,
            row("Accent colour", color(a, "accent", save)),
            h("h4", { text: "Your messages" }), fill(a.outgoing_fill, save),
            h("h4", { text: "Their messages" }), fill(a.incoming_fill, save),
            h("h4", { text: "Chat wallpaper" }), fill(a.chat_background, save)),
          section("Typography", "Fonts, size, spacing", false,
            row("Interface font", fontSelect),
            row("Custom font stack", h("input", { type: "text", value: a.font_family, placeholder: "e.g. 'JetBrains Mono', monospace", onchange: (e) => { a.font_family = e.target.value; save(); } })),
            row("Code font", h("input", { type: "text", value: a.mono_font_family, placeholder: "theme default", onchange: (e) => { a.mono_font_family = e.target.value; save(); } })),
            row("Font size", slider(a, "font_size", 10, 24, " px", save)),
            row("Line height", slider(a, "line_height", 100, 220, "%", save, 5)),
            row("Letter spacing", slider(a, "letter_spacing", -2, 4, "", save))),
          section("Layout", "Density, avatars, sidebar, scaling", false,
            row("Density", segmented(a, "density", [["compact", "Compact"], ["normal", "Normal"], ["comfortable", "Comfortable"]], save)),
            row("Avatar shape", segmented(a, "avatar_shape", [["circle", "Circle"], ["rounded", "Rounded"], ["square", "Square"], ["hidden", "Hidden"]], save)),
            row("Avatar size", slider(a, "avatar_size", 20, 72, " px", save)),
            row("Chat list previews", Kit().sw(a.chat_list_previews, (v) => { a.chat_list_previews = v; save(); })),
            row("Sidebar width", slider(a, "sidebar_width", 200, 520, " px", save, 10)),
            row("Interface scale", slider(a, "ui_scale", 70, 160, "%", save, 5))),
          section("Effects & motion", "Glass, motion, scrollbars", false,
            row("Glass panels", Kit().sw(a.glass_panels, (v) => { a.glass_panels = v; save(); }), "Frosted sidebar and headers; pairs well with a wallpaper."),
            row("Reduce motion", Kit().sw(a.reduced_motion, (v) => { a.reduced_motion = v; save(); }), "Turns off all animations and transitions."),
            row("Styled scrollbars", Kit().sw(a.custom_scrollbars, (v) => { a.custom_scrollbars = v; save(); })),
            row("Scrollbar colour", color(a, "scrollbar_color", save))),
          section("Per-chat appearance", "Different wallpaper and colours for individual chats", false,
            h("div", { class: "mg-toolbar" }, chatPick, h("button", { class: "mg-btn", type: "button", text: "Add override", onclick: () => {
              const id = chatPick.value;
              if (!id) return;
              a.per_chat[id] ??= { background: { enabled: true, from: "#1f2550", to: "#4a3fa8", angle: 160 }, bubble_in: "", bubble_out: "", accent: "" };
              drawPerChat();
              save();
            } })),
            perChatList),
          h("div", { class: "mg-toolbar" },
            h("button", { class: "mg-btn mg-danger", type: "button", text: "Reset Appearance Studio", onclick: async () => {
              if (!(await UI.confirm("Reset appearance?", "All Appearance Studio settings go back to defaults. Themes and QuickCSS are not touched.", "Reset"))) return;
              const fresh = await Bridge.invoke("get_appearance");
              const defaults = Object.fromEntries(Object.keys(fresh).map((k) => [k, undefined]));
              await Bridge.invoke("set_appearance", { appearance: { ...defaults, enabled: true, per_chat: {} } }).catch(() => {});
              window.MelloGram.Settings.show("appearance");
            } }))),
        h("div", { class: "mg-studio-preview" }, h("div", { class: "mg-hint", text: "Live preview" }), preview)),
    ];
  }

  // ---- Workspaces & sharing ---------------------------------------------------------------------

  async function workspacesPage() {
    const list = await Bridge.invoke("list_workspaces");
    const redraw = () => window.MelloGram.Settings.show("workspaces");
    const call = Kit().call;
    const nameInput = h("input", { type: "text", placeholder: "Name, e.g. Work" });
    const iconInput = h("input", { type: "text", placeholder: "🗂", maxlength: 4, class: "mg-icon-input" });

    const cards = list.map((w) => h("div", { class: `mg-workspace${w.active ? " mg-active" : ""}` },
      h("div", { class: "mg-workspace-icon", text: w.icon }),
      h("div", { class: "mg-workspace-info" },
        h("b", { text: w.name }), w.active ? h("span", { class: "mg-tag", text: "active" }) : null,
        h("div", { class: "mg-hint", text: `${w.enabled_plugins} plugins · ${w.themes} themes · updated ${w.updated_at ? new Date(w.updated_at).toLocaleString() : "—"}` }),
        w.description ? h("div", { class: "mg-hint", text: w.description }) : null),
      h("div", { class: "mg-workspace-actions" },
        w.active ? null : h("button", { class: "mg-btn mg-primary", text: "Switch", onclick: async () => { await call("switch_workspace", { id: w.id }, `Switched to ${w.name}`); window.MelloGram.Extras.refreshWorkspaceChip(); redraw(); } }),
        h("button", { class: "mg-icon-btn", title: "Rename / edit", text: "✎", onclick: () => editWorkspace(w) }),
        h("button", { class: "mg-icon-btn", title: "Duplicate", text: "⧉", onclick: async () => { await call("duplicate_workspace", { id: w.id }, "Duplicated"); redraw(); } }),
        h("button", { class: "mg-icon-btn", title: "Share / export", text: "⇪", onclick: () => exportProfile(w) }),
        h("button", { class: "mg-icon-btn", title: "Delete", text: "🗑", onclick: async () => {
          if (await UI.confirm(`Delete ${w.name}?`, w.active ? "Your current setup stays as it is; it just won't be saved to this workspace any more." : "This removes the saved workspace.", "Delete")) {
            await call("delete_workspace", { id: w.id }); window.MelloGram.Extras.refreshWorkspaceChip(); redraw();
          }
        } }))));

    function editWorkspace(w) {
      const name = h("input", { type: "text", value: w.name });
      const icon = h("input", { type: "text", value: w.icon, maxlength: 4, class: "mg-icon-input" });
      const desc = h("textarea", { rows: 3 });
      desc.value = w.description;
      let close;
      close = UI.modal("Edit workspace", [row("Icon", icon), row("Name", name), h("label", { text: "Description" }), desc,
        h("div", { class: "mg-toolbar" }, h("button", { class: "mg-btn mg-primary", text: "Save", onclick: async () => {
          await call("update_workspace", { id: w.id, name: name.value, icon: icon.value, description: desc.value });
          close(); window.MelloGram.Extras.refreshWorkspaceChip(); redraw();
        } }))]);
    }

    function exportProfile(w) {
      const name = h("input", { type: "text", value: w?.name || "My MelloGram setup" });
      const author = h("input", { type: "text", placeholder: "Your name (optional)" });
      const desc = h("textarea", { rows: 3, placeholder: "What makes this setup special?" });
      desc.value = w?.description || "";
      let close;
      close = UI.modal("Share as a profile", [
        h("p", { class: "mg-hint", text: "Creates a .mgprofile.json with the workspace's plugins, plugin settings, themes, QuickCSS, appearance and notification settings. Your Telegram login, credentials and app-lock PIN are never included." }),
        row("Name", name), row("Author", author), h("label", { text: "Description" }), desc,
        h("div", { class: "mg-toolbar" }, h("button", { class: "mg-btn mg-primary", text: "Export", onclick: async () => {
          const json = await call("export_profile", { id: w?.id || null, name: name.value, author: author.value, description: desc.value });
          Kit().download(`${(name.value || "profile").replace(/[^\w.-]+/g, "_")}.mgprofile.json`, json);
          close();
        } }))]);
    }

    async function importProfile() {
      const file = await Kit().pickFile(".json,application/json");
      if (!file) return;
      const json = await file.text();
      let report;
      try { report = await Bridge.invoke("inspect_profile", { json }); }
      catch (e) { UI.toast(String(e), "error"); return; }
      const perm = Kit().PERMISSION_LABELS;
      let close;
      close = UI.modal(`Import “${report.name}”?`, [
        h("p", { class: "mg-hint", text: [report.author && `by ${report.author}`, report.app_version && `made with MelloGram ${report.app_version}`, report.created_at && new Date(report.created_at).toLocaleDateString()].filter(Boolean).join(" · ") }),
        report.description ? h("p", { text: report.description }) : null,
        ...report.warnings.map((w) => h("div", { class: "mg-warning", text: `⚠ ${w}` })),
        h("h4", { text: `Plugins (${report.plugins.filter((p) => p.enabled).length} enabled)` }),
        h("div", { class: "mg-list" }, ...report.plugins.map((p) => h("div", { class: "mg-list-item" },
          h("span", { text: p.enabled ? "✅" : "⬜" }),
          h("div", { class: "mg-list-main" }, h("b", { text: p.name }),
            h("div", { class: "mg-plugin-tags" }, ...p.permissions.map((x) => h("span", { class: "mg-tag", text: perm[x] || x })),
              p.customised_settings ? h("span", { class: "mg-tag", text: `${p.customised_settings} custom settings` }) : null))))),
        report.unknown_plugins.length ? h("p", { class: "mg-hint", text: `Not installed (ignored): ${report.unknown_plugins.join(", ")}` }) : null,
        h("h4", { text: `Themes (${report.themes.length})` }),
        h("p", { class: "mg-hint", text: report.themes.map((t) => `${t.name}${t.enabled ? " (on)" : ""} · ${(t.bytes / 1024).toFixed(1)} KB`).join(" · ") || "none" }),
        h("p", { class: "mg-hint", text: `QuickCSS: ${report.quick_css_bytes} bytes · Appearance Studio: ${report.appearance_customised ? "customised" : "defaults"}` }),
        h("p", { class: "mg-muted", text: "It will be added as a new workspace. Nothing changes until you switch to it." }),
        h("div", { class: "mg-toolbar" },
          h("button", { class: "mg-btn mg-primary", text: "Add workspace", onclick: async () => {
            const r = await call("import_profile", { json }, "Profile added as a workspace");
            if (r.skipped_themes.length) UI.toast(`Kept your existing theme(s): ${r.skipped_themes.join(", ")}`, "warning");
            close(); redraw();
          } }),
          h("button", { class: "mg-btn", text: "Cancel", onclick: () => close() }))]);
    }

    return [
      h("h2", { text: "Workspaces & Sharing" }),
      h("p", { class: "mg-muted", text: "A workspace is a complete setup — plugins and their settings, themes, QuickCSS, appearance and notifications. Switch instantly from the title bar, the command palette (Ctrl+P), or here." }),
      section("New workspace", "Saves your current setup as a workspace and makes it active", !list.length,
        h("div", { class: "mg-toolbar" }, iconInput, nameInput, h("button", { class: "mg-btn mg-primary", text: "Create", onclick: async () => {
          await call("create_workspace", { name: nameInput.value, icon: iconInput.value }, "Workspace created");
          window.MelloGram.Extras.refreshWorkspaceChip(); redraw();
        } })),
        h("div", { class: "mg-toolbar" }, ...[["🏠", "Personal"], ["💼", "Work"], ["🎮", "Gaming"], ["🛡", "Privacy"], ["🧘", "Minimal"], ["♿", "Accessibility"]].map(([i, n]) =>
          h("button", { class: "mg-chip", text: `${i} ${n}`, onclick: () => { iconInput.value = i; nameInput.value = n; } })))),
      ...cards,
      list.length ? null : h("p", { class: "mg-hint", text: "No workspaces yet." }),
      h("h3", { text: "Share" }),
      h("div", { class: "mg-toolbar" },
        h("button", { class: "mg-btn", text: "Export current setup as profile…", onclick: () => exportProfile(null) }),
        h("button", { class: "mg-btn", text: "Import profile…", onclick: importProfile })),
      h("p", { class: "mg-hint", text: "Profiles are inspected before import: you see every plugin they enable, its permissions, and any custom CSS. Imports never overwrite your themes." }),
    ];
  }

  // ---- Privacy & security ------------------------------------------------------------------

  async function privacyPage() {
    const status = await Bridge.invoke("lock_status");
    const plugins = await Bridge.invoke("list_plugins");
    const call = Kit().call;
    const redraw = () => window.MelloGram.Settings.show("privacy");
    const current = h("input", { type: "password", placeholder: "Current PIN", autocomplete: "off" });
    const pin = h("input", { type: "password", placeholder: "New PIN (4+ characters)", autocomplete: "off" });
    const pin2 = h("input", { type: "password", placeholder: "Repeat new PIN", autocomplete: "off" });
    const minutes = h("input", { type: "number", min: 0, max: 1440, value: status.lock_after_minutes });
    let onStart = status.lock_on_start;

    const privacyPlugins = ["privacy-mode", "secret-guard", "link-scrubber", "silent-typing", "anonymise-uploads"];
    const toggles = privacyPlugins.map((id) => plugins.find((p) => p.id === id)).filter(Boolean).map((p) =>
      row(p.name, Kit().sw(p.enabled, async (v, el) => {
        try { await Bridge.invoke("set_plugin_enabled", { id: p.id, enabled: v }); } catch (e) { el.checked = !v; UI.toast(String(e), "error"); }
      }), p.description));

    const perm = Kit().PERMISSION_LABELS;
    const enabled = plugins.filter((p) => p.enabled);
    return [
      h("h2", { text: "Privacy & Security" }),
      h("p", { class: "mg-muted", text: "These controls change what your own screen shows and what this client sends. They cannot stop someone you message from screenshotting, forwarding or copying it." }),
      section("App lock", status.has_pin ? "A PIN is set" : "Protect MelloGram with a PIN", true,
        h("p", { class: "mg-hint", text: "The PIN is stored only as a salted, stretched hash on this device and is never exported. It keeps casual onlookers out; it does not encrypt your Telegram session file." }),
        status.has_pin ? current : null, pin, pin2,
        h("div", { class: "mg-toolbar" },
          h("button", { class: "mg-btn mg-primary", text: status.has_pin ? "Change PIN" : "Set PIN", onclick: async () => {
            if (pin.value !== pin2.value) return UI.toast("PINs don't match", "error");
            await call("set_lock_pin", { current: current.value || null, pin: pin.value }, "PIN saved");
            redraw();
          } }),
          status.has_pin ? h("button", { class: "mg-btn mg-danger", text: "Remove PIN", onclick: async () => { await call("clear_lock_pin", { pin: current.value }, "PIN removed"); redraw(); } }) : null,
          status.has_pin ? h("button", { class: "mg-btn", text: "Lock now (Ctrl+L)", onclick: () => { window.MelloGram.Settings.close(); window.MelloGram.Lock.lockNow(); } }) : null),
        row("Auto-lock after (minutes, 0 = off)", minutes),
        row("Lock when MelloGram starts", Kit().sw(onStart, (v) => { onStart = v; })),
        h("button", { class: "mg-btn", text: "Save lock options", onclick: async () => {
          await call("set_lock_options", { lockAfterMinutes: Number(minutes.value) || 0, lockOnStart: onStart }, "Saved");
          window.MelloGram.Lock.configure(status.has_pin ? Number(minutes.value) || 0 : 0);
        } })),
      section("Privacy tools", "Quick switches for privacy plugins", true, ...toggles,
        h("p", { class: "mg-hint", text: "Fine-tune each in Plugins → ⚙. Ctrl+Shift+H instantly blurs the whole window while Privacy Mode is on." })),
      section("Plugin permissions", `${enabled.length} enabled plugins`, false,
        h("table", { class: "mg-table" },
          h("thead", {}, h("tr", {}, h("th", { text: "Plugin" }), h("th", { text: "Permissions" }))),
          h("tbody", {}, ...enabled.map((p) => h("tr", {}, h("td", { text: p.name }), h("td", { text: p.permissions.map((x) => perm[x] || x).join(", ") || "none" })))))),
      section("What never leaves this device", "By design", false,
        h("ul", {}, ...["Your Telegram login code and 2-step password: they go only to Telegram.", "Your Telegram session file and API hash: never exported, never in backups or profiles.",
          "Your app-lock PIN: only a salted hash, never exported.", "Message History, Bookmarks and Notes: stored locally only.",
          "Diagnostics: local only; support bundles redact credentials. There is no telemetry."].map((t) => h("li", { text: t })))),
    ];
  }

  // ---- Diagnostics --------------------------------------------------------------------------

  const fmtBytes = (b) => (b == null ? "n/a" : b > 1048576 ? `${(b / 1048576).toFixed(1)} MB` : `${(b / 1024).toFixed(1)} KB`);
  const fmtUptime = (s) => `${Math.floor(s / 3600)}h ${Math.floor((s % 3600) / 60)}m ${s % 60}s`;

  async function diagnosticsPage() {
    const d = await Bridge.invoke("get_diagnostics");
    const stat = (label, value, tone) => h("div", { class: `mg-stat ${tone || ""}` }, h("div", { class: "mg-stat-value", text: value }), h("div", { class: "mg-hint", text: label }));
    const testsBox = h("div", { class: "mg-tests" }, h("p", { class: "mg-hint", text: "Run the test suite to check storage, settings, themes, plugins, performance and Telegram connectivity." }));
    const icon = { pass: "✅", warn: "⚠️", fail: "❌", skip: "⏭" };

    async function runTests(btn) {
      btn.disabled = true;
      testsBox.replaceChildren(h("p", { class: "mg-hint", text: "Running…" }));
      try {
        const results = await Bridge.invoke("run_tests");
        const counts = results.reduce((acc, r) => ((acc[r.status] = (acc[r.status] || 0) + 1), acc), {});
        testsBox.replaceChildren(
          h("p", { text: `${counts.pass || 0} passed · ${counts.warn || 0} warnings · ${counts.fail || 0} failed · ${counts.skip || 0} skipped` }),
          h("table", { class: "mg-table" }, h("tbody", {}, ...results.map((r) => h("tr", { class: `mg-test-${r.status}` },
            h("td", { text: icon[r.status] }), h("td", { text: r.name }), h("td", { class: "mg-hint", text: r.detail }), h("td", { class: "mg-hint", text: `${r.ms.toFixed(1)} ms` }))))));
      } catch (e) {
        testsBox.replaceChildren(h("p", { class: "mg-error", text: String(e) }));
      } finally {
        btn.disabled = false;
      }
    }

    const runBtn = h("button", { class: "mg-btn mg-primary", text: "Run all tests", onclick: (e) => runTests(e.target) });
    const tg = d.telegram;
    return [
      h("h2", { text: "Diagnostics" }),
      h("p", { class: "mg-muted", text: "Everything here is computed locally. Nothing is sent anywhere." }),
      h("div", { class: "mg-stats" },
        stat("Uptime", fmtUptime(d.uptime_s)),
        stat("Telegram", !tg.configured ? "not set up" : tg.authorized ? "signed in" : "signed out", tg.authorized ? "good" : "warn"),
        stat("MTProto ping", tg.ping_ms != null ? `${tg.ping_ms.toFixed(0)} ms` : "—", tg.ping_ms != null && tg.ping_ms < 500 ? "good" : ""),
        stat("Messages received", d.counters.messages_received),
        stat("Sent / failed", `${d.counters.sent} / ${d.counters.send_failures}`, d.counters.send_failures ? "warn" : ""),
        stat("Stopped by plugins", d.counters.sends_stopped_by_plugins),
        stat("Reconnects", d.counters.disconnects, d.counters.disconnects ? "warn" : ""),
        stat("Plugins", `${d.plugins.enabled} / ${d.plugins.installed}`),
        stat("Themes", d.themes),
        stat("Storage", fmtBytes(d.storage_bytes)),
        stat("Memory", fmtBytes(d.memory_bytes)),
        stat("Platform", d.platform)),
      d.safe_mode ? h("div", { class: "mg-warning", text: "Running in safe mode." }) : null,
      section("Plugin performance", "Hook timings this session", false,
        h("table", { class: "mg-table" },
          h("thead", {}, h("tr", {}, ...["Plugin", "Calls", "Avg", "Max", "Start", "Errors"].map((t) => h("th", { text: t })))),
          h("tbody", {}, ...d.plugins.stats.sort((a, b) => b.avg_us - a.avg_us).map((p) => h("tr", {},
            h("td", { text: p.name }), h("td", { text: p.calls }), h("td", { text: `${p.avg_us} µs` }), h("td", { text: `${p.max_us} µs` }), h("td", { text: `${p.start_us} µs` }), h("td", { text: p.errors })))))),
      h("h3", { text: "Testing center" }),
      h("div", { class: "mg-toolbar" }, runBtn,
        h("button", { class: "mg-btn", text: "Export support bundle", onclick: async () => {
          const json = await Kit().call("support_bundle");
          Kit().download(`mellogram-support-${new Date().toISOString().slice(0, 10)}.json`, json, "application/json");
        } }),
        h("button", { class: "mg-btn", text: "Restart in safe mode", onclick: () => Bridge.invoke("restart_app", { safeMode: true }) })),
      testsBox,
    ];
  }

  window.MelloGram.Pages = {
    appearance: appearancePage,
    workspaces: workspacesPage,
    privacy: privacyPage,
    diagnostics: diagnosticsPage,
  };
})();
