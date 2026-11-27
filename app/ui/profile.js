// MelloID Profile Studio — local profile editor and live preview.
"use strict";

(() => {
  const { Bridge, UI, h } = window.MelloGram;
  const $ = (id) => document.getElementById(id);
  let profile = null;
  let overlay = null;
  let activePresence = null;

  const safeColor = (value) => /^#(?:[0-9a-f]{3}|[0-9a-f]{6})$/i.test(value || "") ? value : "#35bdb1";
  const safeUrl = (value) => {
    if (!value) return "";
    try {
      const url = new URL(value);
      return ["https:", "http:"].includes(url.protocol) ? url.href : "";
    } catch { return ""; }
  };
  const clone = (value) => JSON.parse(JSON.stringify(value));
  const categories = ["social", "gaming", "media", "creative", "development", "community", "website", "other"];
  const visibilityOptions = ["public", "unlisted", "private"];

  function label(text, input, hint) {
    return h("label", { class: "mg-field melloid-field" },
      h("span", { text }),
      input,
      hint ? h("small", { class: "mg-hint", text: hint }) : null);
  }

  function input(id, value, type = "text", attrs = {}) {
    return h("input", { id, type, value: value ?? "", ...attrs });
  }

  function select(id, value, options) {
    const el = h("select", { id });
    for (const option of options) {
      const value = typeof option === "string" ? option : option.value;
      const title = typeof option === "string" ? option : option.label;
      el.append(h("option", { value, text: title }));
    }
    el.value = value;
    return el;
  }

  function currentDraft() {
    const next = clone(profile);
    next.display_name = $("melloid-name").value.trim();
    next.slug = $("melloid-slug").value.trim().toLowerCase();
    next.headline = $("melloid-headline").value.trim();
    next.visibility = $("melloid-visibility").value;
    next.accent_color = safeColor($("melloid-accent").value);
    next.avatar_url = $("melloid-avatar").value.trim() || null;
    next.banner_url = $("melloid-banner").value.trim() || null;
    next.show_presence = $("melloid-presence").checked;
    next.presence = next.show_presence ? activePresence : null;
    next.bio_sections = [...document.querySelectorAll(".melloid-bio-row")].map((row, index) => ({
      id: row.dataset.id || `bio-${index + 1}`,
      heading: row.querySelector("[data-bio-heading]").value,
      body: row.querySelector("[data-bio-body]").value,
      visible: row.querySelector("[data-bio-visible]").checked,
    }));
    next.connections = [...document.querySelectorAll(".melloid-connection-row")].map((row, index) => ({
      id: row.dataset.id || `connection-${index + 1}`,
      provider: row.querySelector("[data-connection-provider]").value.trim(),
      kind: row.querySelector("[data-connection-kind]").value,
      display_name: row.querySelector("[data-connection-name]").value.trim(),
      profile_url: row.querySelector("[data-connection-url]").value.trim() || null,
      verification_issuer: null,
      verified_at: null,
      visible: row.querySelector("[data-connection-visible]").checked,
    }));
    next.badges = [...document.querySelectorAll(".melloid-badge-row")].map((row, index) => ({
      id: row.dataset.id || `collectible-${index + 1}`,
      issuer: "self",
      title: row.querySelector("[data-badge-title]").value.trim(),
      description: row.querySelector("[data-badge-description]").value.trim(),
      category: "collectible",
      awarded_at: null,
      visible: row.querySelector("[data-badge-visible]").checked,
      server_issued: false,
    }));
    return next;
  }

  function renderPreview(target, data) {
    const accent = safeColor(data.accent_color);
    const bannerUrl = safeUrl(data.banner_url);
    const avatarUrl = safeUrl(data.avatar_url);
    const initials = (data.display_name || "M").split(/\s+/).slice(0, 2).map((part) => part[0] || "").join("").toUpperCase();
    const visibleSections = (data.bio_sections || []).filter((section) => section.visible);
    const connections = (data.connections || []).filter((connection) => connection.visible);
    const activity = data.show_presence && data.presence ? data.presence : null;
    const badges = (data.badges || []).filter((badge) => badge.visible);
    const card = h("article", { class: "melloid-preview-card", style: `--melloid-accent:${accent}` },
      h("div", { class: "melloid-preview-banner" },
        bannerUrl ? h("img", { src: bannerUrl, alt: "Profile banner" }) : h("span", { text: "MelloID" })),
      h("div", { class: "melloid-preview-identity" },
        avatarUrl ? h("img", { class: "melloid-preview-avatar", src: avatarUrl, alt: "Avatar" })
          : h("div", { class: "melloid-preview-avatar melloid-preview-initials", text: initials }),
        h("div", { class: "melloid-preview-name" },
          h("h2", { text: data.display_name || "Your name" }),
          h("div", { class: "melloid-preview-slug", text: `@${data.slug || "your-melloid"}` }),
          data.headline ? h("p", { text: data.headline }) : null)),
      h("div", { class: "melloid-preview-content" },
        h("div", { class: "melloid-preview-meta" },
          h("span", { class: "mg-tag", text: `◉ ${data.visibility || "public"}` }),
          data.show_presence ? h("span", { class: "mg-tag", text: "Presence enabled" }) : h("span", { class: "mg-tag", text: "Presence hidden" })),
        activity ? h("section", { class: "melloid-preview-section melloid-live-activity" },
          h("h3", { text: "Live activity" }),
          h("b", { text: activity.activity || "Active" }),
          h("p", { text: [activity.application, activity.details, activity.state].filter(Boolean).join(" · ") }),
          activity.party_current != null && activity.party_max != null
            ? h("span", { class: "mg-tag", text: `Party ${activity.party_current}/${activity.party_max}` }) : null) : null,
        ...visibleSections.map((section) => h("section", { class: "melloid-preview-section" },
          h("h3", { text: section.heading || "About" }),
          h("p", { text: section.body || "Add something about yourself…" }))),
        h("section", { class: "melloid-preview-section" },
          h("h3", { text: "Connections" }),
          connections.length ? h("div", { class: "melloid-preview-connections" },
            ...connections.map((connection) => h("div", { class: "melloid-preview-connection" },
              h("b", { text: connection.provider || "Service" }),
              h("span", { text: connection.display_name || "Connected account" }),
              connection.verification_issuer ? h("small", { class: "melloid-verified-label", text: "✓ Verified" }) : null,
              connection.profile_url && safeUrl(connection.profile_url)
                ? h("a", { href: safeUrl(connection.profile_url), target: "_blank", rel: "noopener noreferrer", text: "Open ↗" }) : null)))
            : h("p", { class: "mg-muted", text: "No public connections added yet." })),
        h("section", { class: "melloid-preview-section" },
          h("h3", { text: "Badges" }),
          badges.length ? h("div", { class: "melloid-preview-badges" },
            ...badges.slice(0, 8).map((badge) => h("span", { class: "melloid-badge-pill", text: `✦ ${badge.title}` })))
            : h("p", { class: "mg-muted", text: "Earned and official badges will appear here when the badge service is connected." }))));
    target.replaceChildren(card,
      h("p", { class: "melloid-preview-disclaimer", text: "Preview only. Public hosting and profile URLs are not live yet; visibility is saved as your intended audience." }));
  }

  function renderEditor() {
    const body = overlay.querySelector(".melloid-editor-scroll");
    const preview = overlay.querySelector(".melloid-preview");
    body.replaceChildren();

    body.append(
      h("div", { class: "melloid-section-title" }, h("h2", { text: "Identity" }), h("p", { class: "mg-muted", text: "Your MelloID is separate from your Telegram username." })),
      label("Display name", input("melloid-name", profile.display_name, "text", { maxlength: 64 })),
      label("Public handle", input("melloid-slug", profile.slug, "text", { maxlength: 32, placeholder: "your-name" }), "Lowercase letters, numbers, and hyphens. Must be unique when public hosting is enabled."),
      label("Headline", input("melloid-headline", profile.headline, "text", { maxlength: 120, placeholder: "A short line about you" })),
      label("Profile visibility", select("melloid-visibility", profile.visibility, visibilityOptions.map((v) => ({ value: v, label: v[0].toUpperCase() + v.slice(1) }))), "This is your intended audience setting. Publishing is not available until MelloID hosting is deployed."),
      label("Accent color", input("melloid-accent", profile.accent_color, "color", { class: "melloid-color-input" })),
      label("Avatar image URL", input("melloid-avatar", profile.avatar_url || "", "url", { placeholder: "https://…" })),
      label("Banner image URL", input("melloid-banner", profile.banner_url || "", "url", { placeholder: "https://…" })),
      h("label", { class: "mg-field-row melloid-presence-toggle" },
        h("span", {}, h("b", { text: "Show activity presence" }), h("small", { class: "mg-hint", text: "Lets your profile display supported MelloGram activity." })),
        h("input", { id: "melloid-presence", type: "checkbox", class: "mg-switch", checked: profile.show_presence })),
      h("div", { class: "melloid-section-title" }, h("h2", { text: "Bio Studio" }), h("p", { class: "mg-muted", text: "Create separate sections instead of one oversized biography." })),
      h("div", { id: "melloid-bio-list", class: "melloid-editor-list" }),
      h("button", { class: "mg-btn", type: "button", text: "+ Add bio section", onclick: () => {
        profile = currentDraft();
        profile.bio_sections.push({ id: `bio-${Date.now()}`, heading: "New section", body: "", visible: true });
        renderEditor();
      }}),
      h("div", { class: "melloid-section-title" }, h("h2", { text: "Connections" }), h("p", { class: "mg-muted", text: "Add accounts you want to display. A link is not an ownership verification." })),
      h("div", { id: "melloid-connection-list", class: "melloid-editor-list" }),
      h("button", { class: "mg-btn", type: "button", text: "+ Add connection", onclick: () => {
        profile = currentDraft();
        profile.connections.push({ id: `connection-${Date.now()}`, provider: "", kind: "social", display_name: "", profile_url: null, verification_issuer: null, verified_at: null, visible: true });
        renderEditor();
      }}),
      h("div", { class: "melloid-section-title" }, h("h2", { text: "Badge showcase" }), h("p", { class: "mg-muted", text: "Create personal collectible labels. Verified, official, achievement, and community badges are granted by trusted issuers only." })),
      h("div", { id: "melloid-badge-list", class: "melloid-editor-list" }),
      h("button", { class: "mg-btn", type: "button", text: "+ Add collectible badge", onclick: () => {
        profile = currentDraft();
        const collectibleCount = profile.badges.filter((badge) => badge.category === "collectible" && !badge.server_issued).length;
        if (collectibleCount >= 24) {
          UI.toast("You can showcase up to 24 personal collectibles.", "warning");
          return;
        }
        profile.badges.push({ id: `collectible-${Date.now()}`, issuer: "self", title: "New collectible", description: "", category: "collectible", awarded_at: null, visible: true, server_issued: false });
        renderEditor();
      }}),
    );

    const bioList = $("melloid-bio-list");
    for (const [index, section] of profile.bio_sections.entries()) {
      bioList.append(h("div", { class: "melloid-list-card melloid-bio-row", "data-id": section.id },
        h("div", { class: "melloid-row-head" }, h("b", { text: `Bio section ${index + 1}` }),
          h("button", { class: "mg-icon-btn mg-danger", type: "button", text: "Remove", onclick: () => { profile = currentDraft(); profile.bio_sections.splice(index, 1); renderEditor(); } })),
        h("input", { "data-bio-heading": "", value: section.heading, maxlength: 48, placeholder: "Section heading" }),
        h("textarea", { "data-bio-body": "", rows: 4, maxlength: 2000, placeholder: "Write about yourself…", text: section.body }),
        h("label", { class: "melloid-inline-check" }, h("input", { "data-bio-visible": "", type: "checkbox", checked: section.visible }), " Show on profile")));
    }
    if (!profile.bio_sections.length) bioList.append(h("p", { class: "mg-muted", text: "No bio sections yet." }));

    const connectionList = $("melloid-connection-list");
    for (const [index, connection] of profile.connections.entries()) {
      const kind = h("select", { "data-connection-kind": "" },
        ...categories.map((category) => h("option", { value: category, text: category[0].toUpperCase() + category.slice(1) })));
      kind.value = connection.kind || "other";
      connectionList.append(h("div", { class: "melloid-list-card melloid-connection-row", "data-id": connection.id },
        h("div", { class: "melloid-row-head" }, h("b", { text: `Connection ${index + 1}` }),
          h("button", { class: "mg-icon-btn mg-danger", type: "button", text: "Remove", onclick: () => { profile = currentDraft(); profile.connections.splice(index, 1); renderEditor(); } })),
        connection.verification_issuer ? h("small", { class: "melloid-verified-label", text: "✓ Verified by a trusted MelloID issuer. Editing account identity details removes this claim." }) : null,
        h("input", { "data-connection-provider": "", value: connection.provider, maxlength: 40, placeholder: "Service (e.g. GitHub)" }),
        kind,
        h("input", { "data-connection-name": "", value: connection.display_name, maxlength: 80, placeholder: "Account display name" }),
        h("input", { "data-connection-url": "", value: connection.profile_url || "", placeholder: "https://profile.example" }),
        h("label", { class: "melloid-inline-check" }, h("input", { "data-connection-visible": "", type: "checkbox", checked: connection.visible }), " Show publicly")));
    }
    if (!profile.connections.length) connectionList.append(h("p", { class: "mg-muted", text: "No connections added yet." }));

    const badgeList = $("melloid-badge-list");
    const collectibleBadges = profile.badges.filter((badge) => badge.category === "collectible" && !badge.server_issued);
    const trustedBadges = profile.badges.filter((badge) => badge.server_issued || badge.category !== "collectible");
    if (trustedBadges.length) {
      badgeList.append(h("div", { class: "melloid-list-card melloid-trusted-badges" },
        h("b", { text: "Issuer-awarded badges (read-only)" }),
        ...trustedBadges.map((badge) => h("div", { class: "melloid-trusted-badge" },
          h("span", { text: badge.title }),
          h("small", { text: `${badge.category} · ${badge.issuer || "Trusted issuer"}` })))));
    }
    for (const [index, badge] of collectibleBadges.entries()) {
      badgeList.append(h("div", { class: "melloid-list-card melloid-badge-row", "data-id": badge.id },
        h("div", { class: "melloid-row-head" },
          h("b", { text: `Custom collectible ${index + 1}` }),
          h("button", { class: "mg-icon-btn mg-danger", type: "button", text: "Remove", onclick: () => { profile = currentDraft(); profile.badges.splice(index, 1); renderEditor(); } })),
        h("input", { "data-badge-title": "", value: badge.title || "", maxlength: 48, placeholder: "Badge title" }),
        h("input", { "data-badge-description": "", value: badge.description || "", maxlength: 160, placeholder: "Description (optional)" }),
        h("label", { class: "melloid-inline-check" },
          h("input", { "data-badge-visible": "", type: "checkbox", checked: badge.visible }), " Show on profile")));
    }
    if (!collectibleBadges.length) badgeList.append(h("p", { class: "mg-muted", text: "No personal collectibles yet." }));

    body.oninput = previewUpdate;
    body.onchange = previewUpdate;
    function previewUpdate() {
      try { renderPreview(preview, currentDraft()); } catch (e) { console.warn("MelloID preview", e); }
    }
    previewUpdate();
  }


  function escapeHtml(value) {
    return String(value ?? "").replace(/[&<>"']/g, (char) => ({
      "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;",
    })[char]);
  }

  function exportPublicHtml() {
    const data = currentDraft();
    if (data.visibility !== "public") {
      UI.toast("Set profile visibility to Public before exporting a public page.", "warning");
      return;
    }
    const accent = safeColor(data.accent_color);
    const avatar = safeUrl(data.avatar_url);
    const banner = safeUrl(data.banner_url);
    const sections = (data.bio_sections || []).filter((item) => item.visible)
      .map((item) => `<section><h2>${escapeHtml(item.heading)}</h2><p>${escapeHtml(item.body)}</p></section>`).join("");
    const connections = (data.connections || []).filter((item) => item.visible)
      .map((item) => {
        const href = safeUrl(item.profile_url);
        const link = href ? `<a href="${escapeHtml(href)}" target="_blank" rel="noopener noreferrer">Visit ↗</a>` : "";
        return `<li><b>${escapeHtml(item.provider)}</b><span>${escapeHtml(item.display_name)}</span>${link}</li>`;
      }).join("");
    const badges = (data.badges || []).filter((item) => item.visible)
      .map((item) => `<span class="badge">${escapeHtml(item.title)}</span>`).join("");
    const activity = data.show_presence && data.presence
      ? `<section><h2>Activity</h2><p>${escapeHtml(data.presence.application)} · ${escapeHtml(data.presence.activity)}</p></section>`
      : "";
    const avatarMarkup = avatar
      ? `<img class="avatar" src="${escapeHtml(avatar)}" alt="Profile avatar">`
      : `<div class="avatar initials">${escapeHtml((data.display_name || "M").split(/\s+/).slice(0, 2).map((part) => part[0] || "").join("").toUpperCase())}</div>`;
    const bannerMarkup = banner
      ? '<div class="banner"><img src="' + escapeHtml(banner) + '" alt="Profile banner"></div>'
      : '<div class="banner"></div>';
    const html = `<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>${escapeHtml(data.display_name)} · MelloID</title>
<meta name="description" content="${escapeHtml(data.headline || "MelloID public profile")}">
<style>
:root{color-scheme:dark;--accent:${accent};font-family:Inter,system-ui,sans-serif;background:#10121c;color:#f4f5fa}
*{box-sizing:border-box}body{margin:0;padding:32px 16px;background:radial-gradient(ellipse at top,color-mix(in srgb,var(--accent) 14%,transparent),transparent 55%),#10121c}
main{max-width:760px;margin:auto;background:#1a1d2b;border:1px solid #303449;border-radius:22px;overflow:hidden;box-shadow:0 24px 80px #0008}
.banner{height:190px;background:linear-gradient(120deg,var(--accent),#171a29);background-size:cover;background-position:center}.banner img{width:100%;height:100%;object-fit:cover}
.identity{padding:0 28px 18px}.avatar{width:112px;height:112px;object-fit:cover;border:5px solid #1a1d2b;border-radius:28px;margin-top:-54px;background:var(--accent)}
.initials{display:grid;place-items:center;font-size:34px;font-weight:800;color:white}
h1{margin:10px 0 0;font-size:30px}.slug{color:#aeb4c9;font-family:ui-monospace,monospace}.headline{color:#c4c9d9}
.content{padding:0 28px 30px}section{border-top:1px solid #303449;padding:18px 0}h2{font-size:16px;color:var(--accent);margin:0 0 9px}p{white-space:pre-wrap;line-height:1.65;color:#c8ccda;margin:0;overflow-wrap:anywhere}
ul{list-style:none;padding:0;margin:0;display:grid;gap:8px}li{display:grid;grid-template-columns:1fr auto;gap:3px 12px;padding:12px;background:#24283a;border-radius:12px}li span{color:#aeb4c9;font-size:14px}li a{grid-column:2;grid-row:1/3;align-self:center;color:var(--accent);text-decoration:none}
.badges{display:flex;flex-wrap:wrap;gap:8px}.badge{padding:6px 10px;border:1px solid var(--accent);border-radius:20px;color:var(--accent);font-size:13px}
footer{padding:16px 28px;border-top:1px solid #303449;color:#858da6;font-size:12px}
@media(max-width:520px){body{padding:10px}.banner{height:130px}.identity,.content{padding-left:18px;padding-right:18px}h1{font-size:24px}}
</style></head>
<body><main>${bannerMarkup}<div class="identity">${avatarMarkup}<h1>${escapeHtml(data.display_name)}</h1>
<div class="slug">@${escapeHtml(data.slug)}</div><p class="headline">${escapeHtml(data.headline)}</p></div>
<div class="content">${sections}${activity}
<section><h2>Connections</h2>${connections ? `<ul>${connections}</ul>` : "<p>No public connections.</p>"}</section>
<section><h2>Badges</h2><div class="badges">${badges || "<p>No public badges yet.</p>"}</div></section></div>
<footer>Published as a static MelloID export · Profile owner controls the information included.</footer></main></body></html>`;
    const url = URL.createObjectURL(new Blob([html], { type: "text/html;charset=utf-8" }));
    const link = h("a", { href: url, download: `melloid-${data.slug || "profile"}.html` });
    link.click();
    setTimeout(() => URL.revokeObjectURL(url), 1500);
    UI.toast("Public profile HTML exported. Review it before uploading or sharing.", "success");
  }

  async function open() {
    try {
      profile = await Bridge.invoke("get_melloid_profile");
      activePresence = profile.presence || null;
      overlay = h("div", { class: "mg-overlay mg-melloid-overlay" });
      let onEscape;
      const close = () => {
        overlay?.remove();
        overlay = null;
        if (onEscape) document.removeEventListener("keydown", onEscape);
      };
      const save = async () => {
        try {
          const next = currentDraft();
          const saved = await Bridge.invoke("save_melloid_profile", { profile: next });
          profile = saved || next;
          UI.toast("MelloID profile saved on this device.", "success");
          renderEditor();
        } catch (error) {
          UI.toast(String(error), "error");
        }
      };
      const editor = h("div", { class: "mg-card mg-melloid-editor" },
        h("header", { class: "melloid-editor-header" },
          h("div", {}, h("strong", { text: "MelloID Studio" }), h("small", { text: "Your identity, your layout, your audience." })),
          h("div", { class: "mg-toolbar" },
            h("button", { class: "mg-btn", text: "Export Public HTML", onclick: exportPublicHtml }),
            h("button", { class: "mg-btn", text: "Export JSON", onclick: () => {
              const data = JSON.stringify(currentDraft(), null, 2);
              const url = URL.createObjectURL(new Blob([data], { type: "application/json" }));
              const link = h("a", { href: url, download: `melloid-${profile.slug || "profile"}.json` });
              link.click(); setTimeout(() => URL.revokeObjectURL(url), 1500);
            }}),
            h("button", { class: "mg-btn mg-primary", text: "Save profile", onclick: save }),
            h("button", { class: "mg-icon-btn", text: "✕", title: "Close", onclick: close }))),
        h("div", { class: "melloid-editor-layout" },
          h("div", { class: "melloid-editor-scroll" }),
          h("aside", { class: "melloid-preview" })));
      overlay.append(editor);
      overlay.addEventListener("click", (event) => { if (event.target === overlay) close(); });
      onEscape = (event) => { if (event.key === "Escape" && overlay) close(); };
      document.addEventListener("keydown", onEscape);
      document.body.append(overlay);
      renderEditor();
    } catch (error) {
      UI.toast(`Could not open MelloID: ${error}`, "error");
    }
  }

  Bridge.listen("mellorpc://presence", (activity) => {
    activePresence = activity || null;
    if (overlay && profile) {
      try { renderPreview(overlay.querySelector(".melloid-preview"), currentDraft()); }
      catch (error) { console.warn("MelloID presence preview", error); }
    }
  });

  const button = h("button", { class: "mg-icon-btn", id: "open-melloid", title: "Open MelloID profile studio", text: "ID", onclick: open });
  $("header-buttons").append(button);
  window.MelloGram.MelloID = { open };
})();
