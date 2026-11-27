// Message media: photos, albums, GIFs, videos, stickers (static, Lottie TGS,
// WebM), voice notes, audio, files, locations, venues, web previews,
// contacts, polls and dice. Files come from the app's media cache through the
// scoped asset protocol; nothing is fetched from third parties except opt-in
// map tiles.
"use strict";

(() => {
  const { Bridge, UI, h } = window.MelloGram;
  let settings = { auto_download_mb: 8, autoplay_gifs: true, autoplay_video_stickers: true, load_map_tiles: false, respect_spoilers: true };
  Bridge.invoke("get_media_settings").then((s) => { if (s) settings = s; }).catch(() => {});

  const fmtSize = (b) => (b == null ? "" : b >= 1048576 ? `${(b / 1048576).toFixed(1)} MB` : b >= 1024 ? `${Math.round(b / 1024)} KB` : `${b} B`);
  const fmtDur = (s) => { if (s == null) return ""; s = Math.round(s); return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`; };

  const cache = new Map(); // key -> Promise<url|null>
  function fileUrl(m, variant) {
    const key = `${m.chat_id}:${m.id}:${variant}`;
    if (!cache.has(key)) {
      cache.set(key, Bridge.invoke("media_file", { chatId: m.chat_id, messageId: m.id, variant, mime: m.media?.mime || null })
        .then((f) => f?.url || null)
        .catch((e) => { cache.delete(key); throw e; }));
    }
    return cache.get(key);
  }

  const autoOk = (m) => settings.auto_download_mb > 0 && (m.media.size || 0) <= settings.auto_download_mb * 1048576;

  // Load when visible.
  const io = new IntersectionObserver((entries) => {
    for (const e of entries) if (e.isIntersecting) { io.unobserve(e.target); e.target.__load?.(); }
  }, { rootMargin: "400px" });
  const whenVisible = (el, fn) => { el.__load = fn; io.observe(el); return el; };

  function errorBox(e) { return h("div", { class: "mg-media-error", text: `Could not load media: ${e}` }); }

  function sized(el, md, max = 360) {
    if (md.width && md.height) {
      const scale = Math.min(1, max / md.width, max / md.height);
      el.style.width = `${Math.round(md.width * scale)}px`;
      el.style.aspectRatio = `${md.width} / ${md.height}`;
    }
    return el;
  }

  function spoilerWrap(el, md) {
    if (!(md.spoiler && settings.respect_spoilers)) return el;
    el.classList.add("mg-spoiler");
    el.title = "Spoiler: click to reveal";
    el.addEventListener("click", (e) => { if (el.classList.contains("mg-spoiler")) { e.stopPropagation(); el.classList.remove("mg-spoiler"); } }, { capture: true });
    return el;
  }

  function downloadButton(m, label = "Download") {
    return h("button", { class: "mg-btn mg-btn-small", text: `${label}${m.media.size ? ` (${fmtSize(m.media.size)})` : ""}`, onclick: async (e) => {
      e.stopPropagation();
      const btn = e.currentTarget;
      btn.disabled = true; btn.textContent = "Saving…";
      try {
        const r = await Bridge.invoke("save_media", { chatId: m.chat_id, messageId: m.id, mime: m.media.mime || null, fileName: m.media.file_name || null });
        UI.toast(`Saved to ${r.path}${r.risky ? " — this file type can run code; only open it if you trust the sender." : ""}`, r.risky ? "warning" : "success");
        btn.textContent = "Saved";
      } catch (err) {
        UI.toast(String(err), "error"); btn.disabled = false; btn.textContent = label;
      }
    } });
  }

  function photo(m) {
    const md = m.media;
    const box = spoilerWrap(sized(h("div", { class: "mg-photo" }), md), md);
    const load = async (full) => {
      try {
        const thumb = md.has_thumbnail ? await fileUrl(m, "thumb") : null;
        if (thumb && !box.firstChild) box.append(h("img", { src: thumb, alt: "Photo preview", class: "mg-blur-up" }));
        if (!full) return;
        const url = await fileUrl(m, "full");
        if (url) box.replaceChildren(h("img", { src: url, alt: m.text ? m.text.slice(0, 80) : "Photo", onclick: () => viewer(url, "img") }));
      } catch (e) { box.replaceChildren(errorBox(e)); }
    };
    if (autoOk(m)) whenVisible(box, () => load(true));
    else { whenVisible(box, () => load(false)); box.append(h("button", { class: "mg-media-load", text: `Load photo ${fmtSize(md.size)}`, onclick: (e) => { e.stopPropagation(); e.target.remove(); load(true); } })); }
    return box;
  }

  function video(m, { loop = false, round = false } = {}) {
    const md = m.media;
    const box = spoilerWrap(sized(h("div", { class: `mg-video${round ? " mg-round" : ""}` }), md, round ? 240 : 360), md);
    const play = async () => {
      box.classList.add("mg-loading");
      try {
        const url = await fileUrl(m, "full");
        if (!url) return;
        const v = h("video", { src: url, controls: !loop, playsInline: true, preload: "metadata" });
        if (loop) { v.loop = true; v.muted = true; v.autoplay = settings.autoplay_gifs; v.addEventListener("click", () => (v.paused ? v.play() : v.pause())); }
        box.replaceChildren(v);
        if (!loop) v.play().catch(() => {});
      } catch (e) { box.replaceChildren(errorBox(e)); } finally { box.classList.remove("mg-loading"); }
    };
    const poster = async () => { try { const t = md.has_thumbnail ? await fileUrl(m, "thumb") : null; if (t && box.querySelector(".mg-play")) box.prepend(h("img", { src: t, alt: "Video preview" })); } catch { /* no thumbnail */ } };
    box.append(h("button", { class: "mg-play", title: "Play", "aria-label": `Play video ${fmtDur(md.duration)}`, onclick: (e) => { e.stopPropagation(); play(); } }, h("span", { text: "▶" }), h("small", { text: [fmtDur(md.duration), fmtSize(md.size)].filter(Boolean).join(" · ") })));
    if (loop && autoOk(m)) whenVisible(box, play); else whenVisible(box, poster);
    return box;
  }

  async function lottie(m, box) {
    try {
      const text = await Bridge.invoke("media_lottie", { chatId: m.chat_id, messageId: m.id });
      if (!text || !window.lottie) return;
      const data = JSON.parse(text);
      box.replaceChildren();
      window.lottie.loadAnimation({ container: box, renderer: "svg", loop: true, autoplay: true, animationData: data });
    } catch (e) { box.replaceChildren(h("span", { class: "mg-sticker-emoji", text: m.media.emoji || "🙂" })); }
  }

  function sticker(m) {
    const md = m.media;
    const box = h("div", { class: "mg-sticker", title: md.emoji ? `Sticker ${md.emoji}` : "Sticker" }, h("span", { class: "mg-sticker-emoji", text: md.emoji || "🙂" }));
    if (md.kind === "animated-sticker") whenVisible(box, () => lottie(m, box));
    else if (md.kind === "video-sticker") whenVisible(box, async () => {
      try { const url = await fileUrl(m, "full"); if (url) { const v = h("video", { src: url, playsInline: true }); v.loop = true; v.muted = true; v.autoplay = settings.autoplay_video_stickers; box.replaceChildren(v); } } catch { /* keep emoji */ }
    });
    else whenVisible(box, async () => { try { const url = await fileUrl(m, "full"); if (url) box.replaceChildren(h("img", { src: url, alt: `Sticker ${md.emoji || ""}` })); } catch { /* keep emoji */ } });
    return box;
  }

  function audio(m) {
    const md = m.media;
    const voice = md.kind === "voice";
    const box = h("div", { class: `mg-audio${voice ? " mg-voice" : ""}` });
    const label = voice ? `Voice message · ${fmtDur(md.duration)}` : [md.performer, md.title || md.file_name].filter(Boolean).join(" — ") || "Audio";
    const btn = h("button", { class: "mg-play-round", title: "Play", "aria-label": `Play ${label}`, text: "▶" });
    box.append(btn, h("div", { class: "mg-audio-meta" }, h("b", { text: label }), h("small", { class: "mg-muted", text: [voice ? "" : fmtDur(md.duration), fmtSize(md.size)].filter(Boolean).join(" · ") })));
    btn.onclick = async (e) => {
      e.stopPropagation();
      btn.disabled = true; btn.textContent = "…";
      try {
        const url = await fileUrl(m, "full");
        const a = h("audio", { src: url, controls: true, preload: "auto" });
        box.replaceChildren(a, h("small", { class: "mg-muted", text: label }));
        a.play().catch(() => {});
      } catch (err) { box.append(errorBox(err)); btn.disabled = false; btn.textContent = "▶"; }
    };
    if (!voice) box.append(downloadButton(m, "Save"));
    return box;
  }

  function documentCard(m) {
    const md = m.media;
    const ext = (md.file_name || "").split(".").pop()?.slice(0, 5).toUpperCase() || "FILE";
    return h("div", { class: "mg-file" },
      h("div", { class: "mg-file-icon", text: ext }),
      h("div", { class: "mg-file-meta" }, h("b", { text: md.file_name || "File" }), h("small", { class: "mg-muted", text: [fmtSize(md.size), md.mime].filter(Boolean).join(" · ") })),
      downloadButton(m, "Save"));
  }

  // ---- location ------------------------------------------------------------------

  function tileXY(lat, lon, z) {
    const n = 2 ** z;
    const x = ((lon + 180) / 360) * n;
    const r = (lat * Math.PI) / 180;
    const y = ((1 - Math.log(Math.tan(r) + 1 / Math.cos(r)) / Math.PI) / 2) * n;
    return [x, y];
  }

  /** Static map from OSM tiles (opt-in), centred on the point with a pin. */
  function mapView(lat, lon, { z = 15, w = 300, hgt = 180 } = {}) {
    const box = h("div", { class: "mg-map", style: `width:${w}px;height:${hgt}px` });
    if (!settings.load_map_tiles) {
      box.classList.add("mg-map-off");
      box.append(h("span", { text: "🗺" }), h("small", { text: "Map previews are off (Settings → Media). They load tiles from OpenStreetMap." }));
      return box;
    }
    const [fx, fy] = tileXY(lat, lon, z);
    const px = fx * 256, py = fy * 256;
    const left = px - w / 2, top = py - hgt / 2;
    for (let tx = Math.floor(left / 256); tx <= Math.floor((left + w) / 256); tx++) {
      for (let ty = Math.floor(top / 256); ty <= Math.floor((top + hgt) / 256); ty++) {
        const n = 2 ** z;
        if (ty < 0 || ty >= n) continue;
        const img = h("img", { src: `https://tile.openstreetmap.org/${z}/${((tx % n) + n) % n}/${ty}.png`, alt: "", referrerpolicy: "no-referrer", loading: "lazy" });
        img.style.left = `${tx * 256 - left}px`;
        img.style.top = `${ty * 256 - top}px`;
        box.append(img);
      }
    }
    box.append(h("div", { class: "mg-map-pin", text: "📍" }), h("small", { class: "mg-map-attrib", text: "© OpenStreetMap contributors" }));
    return box;
  }

  function location(m) {
    const g = m.media.geo || {};
    const coords = `${g.lat?.toFixed(5)}, ${g.long?.toFixed(5)}`;
    const osm = `https://www.openstreetmap.org/?mlat=${g.lat}&mlon=${g.long}#map=16/${g.lat}/${g.long}`;
    const live = m.media.kind === "live-location";
    const liveLeft = live && g.live_period ? Math.max(0, m.date + g.live_period - Date.now() / 1000) : 0;
    return h("div", { class: "mg-location" },
      mapView(g.lat, g.long),
      h("div", { class: "mg-location-meta" },
        g.title ? h("b", { text: g.title }) : h("b", { text: live ? "Live location" : "Location" }),
        g.address ? h("small", { text: g.address }) : null,
        h("small", { class: "mg-muted", text: `${coords}${g.accuracy_m ? ` · ±${g.accuracy_m} m` : ""}${live ? (liveLeft > 0 ? ` · live for ${Math.round(liveLeft / 60)} more min` : " · live sharing ended") : ""}` }),
        h("div", { class: "mg-toolbar" },
          h("a", { class: "mg-btn mg-btn-small", href: osm, target: "_blank", rel: "noopener noreferrer", text: "Open map" }),
          h("button", { class: "mg-btn mg-btn-small", text: "Copy coordinates", onclick: () => navigator.clipboard.writeText(coords).then(() => UI.toast("Copied", "success")) }))));
  }

  function webPage(m) {
    const w = m.media.web;
    if (!w) return null;
    return h("a", { class: "mg-webpage", href: w.url, target: "_blank", rel: "noopener noreferrer" },
      w.site_name ? h("small", { class: "mg-webpage-site", text: w.site_name }) : null,
      w.title ? h("b", { text: w.title }) : null,
      w.description ? h("span", { text: w.description.length > 240 ? `${w.description.slice(0, 240)}…` : w.description }) : null);
  }

  function render(m) {
    const md = m.media;
    if (!md) return m.has_media ? h("div", { class: "mg-media", text: "📎 Attachment" }) : null;
    switch (md.kind) {
      case "photo": return photo(m);
      case "gif": return video(m, { loop: true });
      case "video": return video(m);
      case "round-video": return video(m, { round: true });
      case "sticker": case "animated-sticker": case "video-sticker": return sticker(m);
      case "voice": case "audio": return audio(m);
      case "document": return documentCard(m);
      case "location": case "live-location": case "venue": return location(m);
      case "web-page": return webPage(m);
      case "contact": return h("div", { class: "mg-contact" }, h("div", { class: "mg-avatar", text: (md.contact?.name || "?").slice(0, 1).toUpperCase() }),
        h("div", {}, h("b", { text: md.contact?.name || "Contact" }), h("div", { class: "mg-muted", text: md.contact?.phone ? `+${md.contact.phone.replace(/^\+/, "")}` : "" })));
      case "poll": return h("div", { class: "mg-poll" }, h("small", { class: "mg-muted", text: `${md.poll?.quiz ? "Quiz" : "Poll"}${md.poll?.closed ? " · closed" : ""}${md.poll?.voters != null ? ` · ${md.poll.voters} votes` : ""}` }),
        h("b", { text: md.poll?.question || "" }), h("ul", {}, ...(md.poll?.options || []).map((o) => h("li", { text: o }))));
      case "dice": return h("div", { class: "mg-dice", title: `Rolled ${md.value}` }, h("span", { text: md.emoji || "🎲" }), h("b", { text: String(md.value ?? "") }));
      default: return h("div", { class: "mg-media", text: "📎 This attachment type is not supported yet" });
    }
  }

  /** Album: several photos/videos sharing a grouped_id, shown as one grid. */
  function album(messages) {
    const grid = h("div", { class: `mg-album mg-album-${Math.min(messages.length, 4)}` });
    for (const m of messages) {
      const el = m.media?.kind === "photo" ? photo(m) : m.media ? render(m) : null;
      if (el) { el.style.width = ""; grid.append(el); }
    }
    return grid;
  }

  // ---- full-screen viewer ----------------------------------------------------------

  function viewer(url, type) {
    const overlay = h("div", { class: "mg-viewer", role: "dialog", "aria-label": "Media viewer" });
    const close = () => { overlay.remove(); document.removeEventListener("keydown", onKey); };
    const onKey = (e) => { if (e.key === "Escape") close(); };
    let scale = 1;
    const media = type === "img" ? h("img", { src: url, alt: "Full-size photo" }) : h("video", { src: url, controls: true, autoplay: true });
    media.addEventListener("wheel", (e) => { e.preventDefault(); scale = Math.min(6, Math.max(1, scale * (e.deltaY < 0 ? 1.15 : 0.87))); media.style.transform = `scale(${scale})`; }, { passive: false });
    overlay.append(media, h("button", { class: "mg-icon-btn mg-viewer-close", text: "✕", title: "Close (Esc)", onclick: close }));
    overlay.addEventListener("click", (e) => { if (e.target === overlay) close(); });
    document.addEventListener("keydown", onKey);
    document.body.append(overlay);
  }

  // ---- composer: share location -------------------------------------------------------

  function shareLocation() {
    const chat = window.MelloGram.App.currentChat();
    if (!chat) return UI.toast("Open a chat first", "info");
    const lat = h("input", { type: "number", step: "any", placeholder: "Latitude, e.g. 51.5074" });
    const lon = h("input", { type: "number", step: "any", placeholder: "Longitude, e.g. -0.1278" });
    const preview = h("div", { class: "mg-location-preview" });
    const update = () => {
      const a = Number(lat.value), b = Number(lon.value);
      preview.replaceChildren(Number.isFinite(a) && Number.isFinite(b) && lat.value && lon.value ? mapView(a, b, { w: 320, hgt: 180 }) : h("small", { class: "mg-muted", text: "Enter coordinates or use your current location." }));
    };
    lat.oninput = lon.oninput = update;
    update();
    let close;
    const useMine = h("button", { class: "mg-btn", text: "Use my current location", onclick: () => {
      if (!navigator.geolocation) return UI.toast("Location is not available on this device", "error");
      useMine.disabled = true; useMine.textContent = "Locating…";
      navigator.geolocation.getCurrentPosition((pos) => {
        lat.value = pos.coords.latitude.toFixed(6); lon.value = pos.coords.longitude.toFixed(6); update();
        useMine.disabled = false; useMine.textContent = "Use my current location";
      }, (err) => { UI.toast(`Could not get your location: ${err.message}`, "error"); useMine.disabled = false; useMine.textContent = "Use my current location"; },
      { enableHighAccuracy: true, timeout: 15000, maximumAge: 60000 });
    } });
    close = UI.modal(`Share a location in ${chat.title}`, [
      h("p", { class: "mg-hint", text: "Your location is only read when you press the button below, and only sent when you press Send." }),
      useMine, h("div", { class: "mg-toolbar" }, lat, lon), preview,
      h("div", { class: "mg-toolbar" }, h("button", { class: "mg-btn mg-primary", text: "Send location", onclick: async () => {
        const a = Number(lat.value), b = Number(lon.value);
        if (!lat.value || !lon.value || !Number.isFinite(a) || !Number.isFinite(b) || Math.abs(a) > 90 || Math.abs(b) > 180) return UI.toast("Enter a valid latitude (−90…90) and longitude (−180…180)", "error");
        try { const msg = await Bridge.invoke("send_location", { chatId: chat.id, lat: a, long: b }); window.MelloGram.App.addMessage?.(msg); close(); UI.toast("Location sent", "success"); }
        catch (e) { UI.toast(String(e), "error"); }
      } }))]);
  }

  const shareBtn = h("button", { class: "mg-icon-btn", title: "Share a location", "aria-label": "Share a location", text: "📍", onclick: shareLocation });
  document.getElementById("composer-buttons")?.append(shareBtn);

  window.MelloGram.Media = { render, album, viewer, mapView, reloadSettings: async () => { settings = await Bridge.invoke("get_media_settings"); } };
})();
