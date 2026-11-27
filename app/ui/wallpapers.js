// MelloStudio wallpapers: runtime (layer elements, parallax) and the
// Settings → Wallpapers page.
"use strict";

(() => {
  const { Bridge, UI, h } = window.MelloGram;
  const $ = (id) => document.getElementById(id);
  const Kit = () => window.MelloGram.SettingsKit;
  const App = () => window.MelloGram.App;

  // ---- runtime -------------------------------------------------------------------

  // Layers the compiled wallpaper CSS targets.
  const chatLayer = h("div", { class: "mg-wallpaper", "aria-hidden": "true" });
  $("chat-view").prepend(chatLayer);
  const windowLayer = h("div", { class: "mg-wallpaper-window", "aria-hidden": "true" });
  document.body.prepend(windowLayer);

  let current = null;

  function effective(chatId) {
    if (!current?.enabled) return null;
    return current.per_chat?.[String(chatId)] || current.global;
  }

  function applyRuntime() {
    const chat = App()?.currentChat?.();
    const anyWallpaper = current?.enabled && (current.global.kind.type !== "none" || current.dark || Object.keys(current.per_chat || {}).length);
    document.body.classList.toggle("mg-has-wallpaper", !!anyWallpaper);
    document.body.classList.toggle("mg-wallpaper-target-window", current?.target === "window");
    const wp = effective(chat?.id);
    document.body.classList.toggle("mg-wallpaper-parallax", !!wp?.parallax);
  }

  async function refresh() {
    current = await Bridge.invoke("get_wallpapers").catch(() => null);
    applyRuntime();
  }

  // Parallax: move the layer a few pixels against the pointer.
  let raf = 0;
  document.addEventListener("pointermove", (e) => {
    if (!document.body.classList.contains("mg-wallpaper-parallax") || matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    cancelAnimationFrame(raf);
    raf = requestAnimationFrame(() => {
      const x = (e.clientX / innerWidth - 0.5) * -14;
      const y = (e.clientY / innerHeight - 0.5) * -14;
      for (const el of [chatLayer, windowLayer]) el.style.transform = `translate(${x.toFixed(1)}px, ${y.toFixed(1)}px) scale(1.04)`;
    });
  }, { passive: true });

  Bridge.listen("mg://wallpapers-changed", refresh);
  window.MelloGram.Wallpapers = { refresh, applyRuntime };
  // Re-evaluate per-chat parallax when the open chat changes.
  document.addEventListener("click", (e) => { if (e.target.closest(".mg-chat-item")) setTimeout(applyRuntime, 50); });
  refresh();

  // ---- settings page ------------------------------------------------------------------

  const TYPES = [["none", "None"], ["solid", "Colour"], ["gradient", "Gradient"], ["image", "Image"], ["pattern", "Pattern"], ["animated", "Animated"]];
  const PATTERNS = ["dots", "grid", "diagonal", "waves", "checker", "hearts", "paws", "stars"];
  const ANIMATIONS = [["aurora", "Aurora"], ["nebula", "Nebula"], ["waves", "Waves"], ["breathe", "Breathe"]];

  function defaultKind(type) {
    switch (type) {
      case "solid": return { type, color: "#1e1d29" };
      case "gradient": return { type, colors: ["#8b7cf6", "#ff8fab"], angle: 160, radial: false };
      case "image": return { type, file: "", fit: "cover" };
      case "pattern": return { type, pattern: "paws", color: "#8b7cf6", background: "#151933", size: 64, opacity: 35 };
      case "animated": return { type, animation: "aurora", colors: ["#1a1446", "#3b2a8f", "#0e5e6f"], speed: 30 };
      default: return { type: "none" };
    }
  }

  function swatch(css) {
    const el = h("div", { class: "mg-wp-swatch" }, h("div", { class: "mg-wp-swatch-layer" }));
    if (css) el.firstChild.style.cssText = css.replace(/inset:[^;]+;/, "");
    return el;
  }

  async function wallpapersPage() {
    const settings = await Bridge.invoke("get_wallpapers");
    const presets = await Bridge.invoke("wallpaper_presets");
    let images = await Bridge.invoke("list_wallpaper_images").catch(() => []);
    const chats = App().chats();
    let slot = "global"; // "global" | "dark" | chat id
    const status = h("span", { class: "mg-hint" });

    let timer;
    const save = () => {
      clearTimeout(timer);
      status.textContent = "…";
      timer = setTimeout(async () => {
        try { await Bridge.invoke("set_wallpapers", { wallpapers: settings }); status.textContent = "Saved · applied live"; updatePreview(); }
        catch (e) { status.textContent = String(e); }
      }, 150);
    };

    const target = () => (slot === "global" ? settings.global : slot === "dark" ? settings.dark : settings.per_chat[slot]);
    const setTarget = (wp) => {
      if (slot === "global") settings.global = wp;
      else if (slot === "dark") settings.dark = wp;
      else settings.per_chat[slot] = wp;
    };
    const ensureTarget = () => {
      if (!target()) setTarget({ kind: { type: "none" }, blur: 0, dim: 0, saturation: 100, parallax: false });
      return target();
    };

    const previewLayer = h("div", { class: "mg-wp-preview-layer" });
    const preview = h("div", { class: "mg-wp-preview" }, previewLayer,
      h("div", { class: "mg-wp-preview-msgs" },
        h("div", { class: "mg-message" }, h("div", { class: "mg-text", text: "How does the new wallpaper look?" })),
        h("div", { class: "mg-message mg-out" }, h("div", { class: "mg-text", text: "Gorgeous ✨" }))));
    async function updatePreview() {
      const wp = target();
      const css = wp ? await Bridge.invoke("wallpaper_preview_css", { wallpaper: wp }) : null;
      previewLayer.style.cssText = css || "";
      previewLayer.style.setProperty("--mg-wp-dim", String((wp?.dim || 0) / 100));
    }

    const editor = h("div");
    const slotPicker = h("select", { onchange: (e) => { slot = e.target.value; drawEditor(); updatePreview(); } });
    function drawSlots() {
      slotPicker.replaceChildren(
        h("option", { value: "global", text: "Everywhere (default)", selected: slot === "global" }),
        h("option", { value: "dark", text: "Dark mode variant", selected: slot === "dark" }),
        ...Object.keys(settings.per_chat).map((id) => h("option", { value: id, selected: slot === id, text: `Chat: ${chats.find((c) => String(c.id) === id)?.title || id}` })));
    }

    const addChat = h("select", {}, h("option", { value: "", text: "Add a per-chat wallpaper…" }),
      ...chats.map((c) => h("option", { value: String(c.id), text: c.title })));
    addChat.addEventListener("change", () => {
      if (!addChat.value) return;
      settings.per_chat[addChat.value] ??= structuredClone(settings.global);
      slot = addChat.value;
      addChat.value = "";
      drawSlots(); drawEditor(); updatePreview(); save();
    });

    const row = (label, control) => h("div", { class: "mg-control" }, h("label", { text: label }), control);
    const range = (obj, key, min, max, unit, onChange = save) => {
      const out = h("output", { text: `${obj[key]}${unit}` });
      return h("div", { class: "mg-slider" }, h("input", { type: "range", min, max, value: obj[key], oninput: (e) => { obj[key] = Number(e.target.value); out.textContent = `${obj[key]}${unit}`; onChange(); } }), out);
    };
    const colorInput = (get, set) => {
      const text = h("input", { type: "text", value: get(), oninput: (e) => { set(e.target.value); save(); } });
      const pick = h("input", { type: "color", value: /^#[0-9a-f]{6}$/i.test(get()) ? get() : "#000000", oninput: (e) => { set(e.target.value); text.value = e.target.value; save(); } });
      return h("div", { class: "mg-color" }, pick, text);
    };

    function drawEditor() {
      const wp = target();
      if (!wp) {
        editor.replaceChildren(h("p", { class: "mg-hint", text: "No dark-mode variant: the default wallpaper is used in dark mode too." }),
          h("button", { class: "mg-btn", text: "Create dark variant", onclick: () => { settings.dark = structuredClone(settings.global); drawEditor(); updatePreview(); save(); } }));
        return;
      }
      const k = wp.kind;
      const typeSeg = h("div", { class: "mg-segmented" }, ...TYPES.map(([v, l]) => h("button", {
        class: k.type === v ? "mg-active" : "", type: "button", text: l,
        onclick: () => { wp.kind = defaultKind(v); drawEditor(); save(); },
      })));
      const fields = [];
      if (k.type === "solid") fields.push(row("Colour", colorInput(() => k.color, (v) => (k.color = v))));
      if (k.type === "gradient" || k.type === "animated") {
        const list = h("div", { class: "mg-wp-colors" });
        const drawColors = () => list.replaceChildren(...k.colors.map((c, i) => h("div", { class: "mg-wp-color" },
          colorInput(() => k.colors[i], (v) => (k.colors[i] = v)),
          k.colors.length > 2 ? h("button", { class: "mg-icon-btn", text: "✕", title: "Remove", onclick: () => { k.colors.splice(i, 1); drawColors(); save(); } }) : null)),
          k.colors.length < (k.type === "gradient" ? 6 : 5) ? h("button", { class: "mg-btn", text: "+ Colour", onclick: () => { k.colors.push("#ffffff"); drawColors(); save(); } }) : null);
        drawColors();
        fields.push(h("div", { class: "mg-field" }, h("label", { text: "Colours" }), list));
      }
      if (k.type === "gradient") {
        fields.push(row("Angle", range(k, "angle", 0, 359, "°")));
        fields.push(row("Radial", Kit().sw(k.radial, (v) => { k.radial = v; save(); })));
      }
      if (k.type === "animated") {
        fields.push(row("Animation", h("select", { onchange: (e) => { k.animation = e.target.value; save(); } }, ...ANIMATIONS.map(([v, l]) => h("option", { value: v, text: l, selected: k.animation === v })))));
        fields.push(row("Cycle length", range(k, "speed", 4, 240, " s")));
        fields.push(h("p", { class: "mg-hint", text: "Animated wallpapers pause automatically when your system asks for reduced motion." }));
      }
      if (k.type === "pattern") {
        fields.push(row("Pattern", h("div", { class: "mg-segmented" }, ...PATTERNS.map((p) => h("button", {
          class: k.pattern === p ? "mg-active" : "", type: "button", text: p, onclick: () => { k.pattern = p; drawEditor(); save(); },
        })))));
        fields.push(row("Pattern colour", colorInput(() => k.color, (v) => (k.color = v))));
        fields.push(row("Background", colorInput(() => k.background, (v) => (k.background = v))));
        fields.push(row("Tile size", range(k, "size", 8, 200, " px")));
        fields.push(row("Pattern opacity", range(k, "opacity", 5, 100, "%")));
      }
      if (k.type === "image") {
        const gallery = h("div", { class: "mg-wp-images" });
        const drawImages = () => gallery.replaceChildren(
          ...images.map((img) => h("div", { class: `mg-wp-image${k.file === img.file ? " mg-active" : ""}` },
            h("img", { src: img.url, alt: "Wallpaper image", loading: "lazy", onclick: () => { k.file = img.file; drawImages(); save(); } }),
            h("button", { class: "mg-icon-btn", title: "Delete image", text: "🗑", onclick: async () => {
              if (!(await UI.confirm("Delete image?", "Wallpapers using it fall back to none.", "Delete"))) return;
              await Kit().call("delete_wallpaper_image", { file: img.file });
              images = images.filter((x) => x.file !== img.file);
              const fresh = await Bridge.invoke("get_wallpapers");
              Object.assign(settings, fresh);
              drawEditor(); updatePreview();
            } }))),
          h("button", { class: "mg-btn", text: "Upload image…", onclick: async () => {
            const f = await Kit().pickFile("image/png,image/jpeg,image/webp,image/gif");
            if (!f) return;
            const data = await new Promise((res, rej) => { const r = new FileReader(); r.onload = () => res(r.result); r.onerror = rej; r.readAsDataURL(f); });
            const img = await Kit().call("import_wallpaper_image", { data }, "Image added");
            images.unshift(img);
            k.file = img.file;
            drawImages(); save();
          } }));
        drawImages();
        fields.push(gallery);
        fields.push(row("Fit", h("div", { class: "mg-segmented" }, ...[["cover", "Fill"], ["contain", "Fit"], ["tile", "Tile"], ["center", "Centre"]].map(([v, l]) => h("button", {
          class: k.fit === v ? "mg-active" : "", type: "button", text: l, onclick: () => { k.fit = v; drawEditor(); save(); },
        })))));
      }
      const effects = k.type === "none" ? [] : [
        h("h4", { text: "Effects" }),
        row("Blur", range(wp, "blur", 0, 40, " px")),
        row("Dim", range(wp, "dim", 0, 90, "%")),
        row("Saturation", range(wp, "saturation", 0, 200, "%")),
        row("Parallax", Kit().sw(wp.parallax, (v) => { wp.parallax = v; save(); applyRuntime(); })),
      ];
      const slotActions = h("div", { class: "mg-toolbar" },
        h("button", { class: "mg-btn", text: "Save as preset", onclick: () => {
          const input = h("input", { type: "text", value: "My wallpaper", maxlength: 60 });
          let close;
          close = UI.modal("Save wallpaper preset", [input, h("div", { class: "mg-toolbar" }, h("button", { class: "mg-btn mg-primary", text: "Save", onclick: () => {
            if (!input.value.trim()) return;
            settings.saved.push({ name: input.value.trim(), wallpaper: structuredClone(wp) });
            save(); close(); UI.toast("Preset saved — it appears in the gallery next time you open this page", "success");
          } }))]);
          input.select();
        } }),
        slot !== "global" ? h("button", { class: "mg-btn mg-danger", text: slot === "dark" ? "Remove dark variant" : "Remove chat wallpaper", onclick: () => {
          if (slot === "dark") settings.dark = null; else delete settings.per_chat[slot];
          slot = "global"; drawSlots(); drawEditor(); updatePreview(); save();
        } }) : null);
      editor.replaceChildren(row("Type", typeSeg), ...fields, ...effects, slotActions);
    }

    const gallery = h("div", { class: "mg-wp-gallery" }, ...presets.map((p) => h("button", {
      class: "mg-wp-preset", type: "button", title: `Use ${p.name}`,
      onclick: () => { setTarget({ ...structuredClone(p.wallpaper), parallax: ensureTarget().parallax }); drawEditor(); updatePreview(); save(); },
    }, swatch(p.css), h("span", { text: p.name }))));

    drawSlots();
    drawEditor();
    updatePreview();

    return [
      h("h2", { text: "Wallpapers" }),
      h("p", { class: "mg-muted", text: "Backgrounds for your chats or the whole window: colours, gradients, your own images, patterns and gentle animations. Blur and dim keep text readable." }),
      h("div", { class: "mg-control" }, h("div", {}, h("label", { text: "Use wallpapers" }), status), Kit().sw(settings.enabled, (v) => { settings.enabled = v; save(); })),
      h("div", { class: "mg-control" }, h("label", { text: "Show behind" }), h("div", { class: "mg-segmented" }, ...[["chat", "Messages"], ["window", "Whole window"]].map(([v, l]) => h("button", {
        class: settings.target === v ? "mg-active" : "", type: "button", text: l,
        onclick: (e) => { settings.target = v; e.target.parentElement.querySelectorAll("button").forEach((b) => b.classList.toggle("mg-active", b === e.target)); save(); },
      })))),
      settings.target === "window" ? h("div", { class: "mg-control" }, h("label", { text: "Panel transparency" }), range(settings, "panel_transparency", 0, 90, "%")) : null,
      h("h3", { text: "Gallery" }),
      gallery,
      h("div", { class: "mg-studio" },
        h("div", { class: "mg-studio-controls" },
          h("div", { class: "mg-toolbar" }, slotPicker, addChat),
          h("section", { class: "mg-section", open: true }, h("div", { class: "mg-section-body" }, editor))),
        h("div", { class: "mg-studio-preview" }, h("div", { class: "mg-hint", text: "Preview" }), preview)),
    ];
  }

  window.MelloGram.Pages.wallpapers = wallpapersPage;
})();
