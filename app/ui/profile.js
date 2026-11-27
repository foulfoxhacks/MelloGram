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
  const profileThemes = [
    { value: "midnight", label: "Midnight" },
    { value: "ocean", label: "Ocean" },
    { value: "aurora", label: "Aurora" },
    { value: "ember", label: "Ember" },
    { value: "rose", label: "Rose" },
    { value: "mint", label: "Mint" },
  ];
  const avatarFrames = [
    { value: "none", label: "No frame" },
    { value: "glow", label: "Soft glow" },
    { value: "neon", label: "Neon ring" },
    { value: "orbit", label: "Orbit" },
    { value: "paw", label: "Paw print" },
  ];
  const profileEffects = [
    { value: "none", label: "No effect" },
    { value: "aurora", label: "Aurora wash" },
    { value: "stars", label: "Starfield" },
    { value: "pawprints", label: "Paw trail" },
    { value: "glow", label: "Ambient glow" },
  ];
  const widgetKinds = [
    { value: "games", label: "Games" },
    { value: "music", label: "Music" },
    { value: "projects", label: "Projects" },
    { value: "communities", label: "Communities" },
    { value: "activity", label: "Activity" },
    { value: "about", label: "About" },
    { value: "custom_link", label: "Custom link" },
  ];

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
    next.profile_theme = $("melloid-profile-theme").value;
    next.avatar_frame = $("melloid-avatar-frame").value;
    next.profile_effect = $("melloid-profile-effect").value;
    next.avatar_url = $("melloid-avatar").value.trim() || null;
    next.banner_url = $("melloid-banner").value.trim() || null;
    next.show_presence = $("melloid-presence").checked;
    next.presence = next.show_presence ? activePresence : null;
    next.widgets = [...document.querySelectorAll(".melloid-widget-row")].map((row, index) => ({
      id: row.dataset.id || `widget-${index + 1}`,
      kind: row.querySelector("[data-widget-kind]").value,
      title: row.querySelector("[data-widget-title]").value.trim()
        || widgetKinds.find((item) => item.value === row.querySelector("[data-widget-kind]").value)?.label || "Widget",
      enabled: row.querySelector("[data-widget-enabled]").checked,
      position: index,
      config: {
        items: row.querySelector("[data-widget-items]").value.split(/\r?\n/)
          .map((item) => item.trim()).filter(Boolean).slice(0, 20),
        url: row.querySelector("[data-widget-url]").value.trim() || null,
      },
    }));
    next.bio_sections = [...document.querySelectorAll(".melloid-bio-row")].map((row, index) => ({
      id: row.dataset.id || `bio-${index + 1}`,
      heading: row.querySelector("[data-bio-heading]").value,
      body: row.querySelector("[data-bio-body]").value,
      visible: row.querySelector("[data-bio-visible]").checked,
    }));
    next.connections = [...document.querySelectorAll(".melloid-connection-row")].map((row, index) => {
      const id = row.dataset.id || `connection-${index + 1}`;
      const provider = row.querySelector("[data-connection-provider]").value.trim();
      const displayName = row.querySelector("[data-connection-name]").value.trim();
      const profileUrl = row.querySelector("[data-connection-url]").value.trim() || null;
      const previous = (profile.connections || []).find((connection) =>
        connection.id === id && connection.provider === provider
          && connection.display_name === displayName && connection.profile_url === profileUrl);
      return {
        id,
        provider,
        kind: row.querySelector("[data-connection-kind]").value,
        display_name: displayName,
        profile_url: profileUrl,
        verification_issuer: previous?.verification_issuer || null,
        verified_at: previous?.verified_at || null,
        visible: row.querySelector("[data-connection-visible]").checked,
      };
    });
    const trustedBadges = (profile.badges || []).filter((badge) =>
      badge.server_issued || badge.category !== "collectible");
    const collectibleBadges = [...document.querySelectorAll(".melloid-badge-row")].map((row, index) => ({
      id: row.dataset.id || `collectible-${index + 1}`,
      issuer: "self",
      title: row.querySelector("[data-badge-title]").value.trim(),
      description: row.querySelector("[data-badge-description]").value.trim(),
      category: "collectible",
      awarded_at: null,
      visible: row.querySelector("[data-badge-visible]").checked,
      server_issued: false,
    }));
    next.badges = [...trustedBadges, ...collectibleBadges];
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
    const widgets = (data.widgets || []).filter((widget) => widget.enabled)
      .sort((a, b) => a.position - b.position);
    const widgetNodes = widgets.map((widget) => {
      const items = Array.isArray(widget.config?.items) ? widget.config.items.filter((item) => typeof item === "string") : [];
      const url = safeUrl(widget.config?.url);
      const title = widget.title || widgetKinds.find((kind) => kind.value === widget.kind)?.label || "Widget";
      return h("section", { class: "melloid-preview-section melloid-widget-preview" },
        h("h3", { text: title }),
        items.length ? h("div", { class: "melloid-widget-items" },
          ...items.map((item) => h("span", { class: "melloid-widget-item", text: item })))
          : url ? h("a", { href: url, target: "_blank", rel: "noopener noreferrer", text: "Open link ↗" })
            : h("p", { class: "mg-muted", text: "This widget is ready for content." }));
    });
    const theme = profileThemes.some((item) => item.value === data.profile_theme) ? data.profile_theme : "midnight";
    const frame = avatarFrames.some((item) => item.value === data.avatar_frame) ? data.avatar_frame : "none";
    const effect = profileEffects.some((item) => item.value === data.profile_effect) ? data.profile_effect : "none";
    const card = h("article", { class: `melloid-preview-card melloid-theme-${theme} melloid-effect-${effect}`, style: `--melloid-accent:${accent}` },
      h("div", { class: "melloid-preview-banner" },
        bannerUrl ? h("img", { src: bannerUrl, alt: "Profile banner" }) : h("span", { text: "MelloID" })),
      h("div", { class: "melloid-preview-identity" },
        avatarUrl ? h("img", { class: `melloid-preview-avatar melloid-frame-${frame}`, src: avatarUrl, alt: "Avatar" })
          : h("div", { class: `melloid-preview-avatar melloid-preview-initials melloid-frame-${frame}`, text: initials }),
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
        ...widgetNodes,
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
      label("Profile theme", select("melloid-profile-theme", profile.profile_theme || "midnight", profileThemes), "Choose a built-in color treatment."),
      label("Avatar frame", select("melloid-avatar-frame", profile.avatar_frame || "none", avatarFrames)),
      label("Profile effect", select("melloid-profile-effect", profile.profile_effect || "none", profileEffects), "Visual-only effect, no custom scripts or CSS."),
      label("Avatar image URL", input("melloid-avatar", profile.avatar_url || "", "url", { placeholder: "https://…" })),
      label("Banner image URL", input("melloid-banner", profile.banner_url || "", "url", { placeholder: "https://…" })),
      h("label", { class: "mg-field-row melloid-presence-toggle" },
        h("span", {}, h("b", { text: "Show activity presence" }), h("small", { class: "mg-hint", text: "Lets your profile display supported MelloGram activity." })),
        h("input", { id: "melloid-presence", type: "checkbox", class: "mg-switch", checked: profile.show_presence })),
      h("div", { class: "melloid-section-title" }, h("h2", { text: "Profile widgets" }), h("p", { class: "mg-muted", text: "Arrange reusable blocks for games, music, projects, communities, activity, and links." })),
      h("div", { id: "melloid-widget-list", class: "melloid-editor-list" }),
      h("div", { class: "mg-toolbar melloid-add-widget" },
        select("melloid-new-widget-type", "games", widgetKinds),
        h("button", { class: "mg-btn", type: "button", text: "+ Add widget", onclick: () => {
          profile = currentDraft();
          if (profile.widgets.length >= 24) {
            UI.toast("You can add up to 24 profile widgets.", "warning");
            return;
          }
          const kind = $("melloid-new-widget-type").value;
          const title = widgetKinds.find((item) => item.value === kind)?.label || "Widget";
          profile.widgets.push({ id: `widget-${Date.now()}`, kind, title, enabled: true, position: profile.widgets.length, config: { items: [], url: null } });
          renderEditor();
        }})),
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

    profile.widgets = [...(profile.widgets || [])].sort((a, b) => a.position - b.position);
    const widgetList = $("melloid-widget-list");
    for (const [index, widget] of profile.widgets.entries()) {
      const kind = h("select", { "data-widget-kind": "" },
        ...widgetKinds.map((item) => h("option", { value: item.value, text: item.label })));
      kind.value = widget.kind || "about";
      const config = widget.config || {};
      const actions = h("div", { class: "mg-toolbar melloid-widget-actions" },
        h("button", { class: "mg-icon-btn", type: "button", text: "↑", title: "Move widget up", disabled: index === 0, onclick: () => {
          profile = currentDraft();
          [profile.widgets[index - 1], profile.widgets[index]] = [profile.widgets[index], profile.widgets[index - 1]];
          profile.widgets.forEach((item, position) => { item.position = position; });
          renderEditor();
        }}),
        h("button", { class: "mg-icon-btn", type: "button", text: "↓", title: "Move widget down", disabled: index === profile.widgets.length - 1, onclick: () => {
          profile = currentDraft();
          [profile.widgets[index + 1], profile.widgets[index]] = [profile.widgets[index], profile.widgets[index + 1]];
          profile.widgets.forEach((item, position) => { item.position = position; });
          renderEditor();
        }}),
        h("button", { class: "mg-icon-btn mg-danger", type: "button", text: "Remove", onclick: () => {
          profile = currentDraft();
          profile.widgets.splice(index, 1);
          profile.widgets.forEach((item, position) => { item.position = position; });
          renderEditor();
        }}));
      widgetList.append(h("div", { class: "melloid-list-card melloid-widget-row", "data-id": widget.id },
        h("div", { class: "melloid-row-head" }, h("b", { text: `Widget ${index + 1}` }), actions),
        kind,
        h("input", { "data-widget-title": "", value: widget.title || "", maxlength: 48, placeholder: "Widget title" }),
        h("textarea", { "data-widget-items": "", rows: 3, maxlength: 2400, placeholder: "One item per line (up to 20)", text: Array.isArray(config.items) ? config.items.join("\n") : "" }),
        h("input", { "data-widget-url": "", value: config.url || "", placeholder: "Optional link (https://…)" }),
        h("label", { class: "melloid-inline-check" },
          h("input", { "data-widget-enabled": "", type: "checkbox", checked: widget.enabled }), " Show on profile")));
    }
    if (!profile.widgets.length) widgetList.append(h("p", { class: "mg-muted", text: "No widgets added. Add one to build your profile layout." }));

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
    const widgets = (data.widgets || []).filter((widget) => widget.enabled).sort((a, b) => a.position - b.position)
      .map((widget) => {
        const title = escapeHtml(widget.title || "Widget");
        const items = Array.isArray(widget.config?.items) ? widget.config.items.filter((item) => typeof item === "string") : [];
        const itemMarkup = items.length ? `<ul>${items.map((item) => `<li>${escapeHtml(item)}</li>`).join("")}</ul>` : "";
        const href = safeUrl(widget.config?.url);
        const linkMarkup = href ? `<a href="${escapeHtml(href)}" target="_blank" rel="noopener noreferrer">Open link ↗</a>` : "";
        return `<section><h2>${title}</h2>${itemMarkup}${linkMarkup}</section>`;
      }).join("");
    const theme = profileThemes.some((item) => item.value === data.profile_theme) ? data.profile_theme : "midnight";
    const frame = avatarFrames.some((item) => item.value === data.avatar_frame) ? data.avatar_frame : "none";
    const effect = profileEffects.some((item) => item.value === data.profile_effect) ? data.profile_effect : "none";
    const avatarMarkup = avatar
      ? `<img class="avatar frame-${frame}" src="${escapeHtml(avatar)}" alt="Profile avatar">`
      : `<div class="avatar initials frame-${frame}">${escapeHtml((data.display_name || "M").split(/\s+/).slice(0, 2).map((part) => part[0] || "").join("").toUpperCase())}</div>`;
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
.theme-ocean{background:linear-gradient(145deg,#142f47,#19273c)}.theme-aurora{background:linear-gradient(145deg,#1b2747,#321d42)}.theme-ember{background:linear-gradient(145deg,#3a241d,#241b2e)}.theme-rose{background:linear-gradient(145deg,#382139,#241b30)}.theme-mint{background:linear-gradient(145deg,#19382f,#1d2b34)}
.frame-glow{box-shadow:0 0 22px var(--accent)}.frame-neon{border-color:var(--accent);box-shadow:0 0 0 2px var(--accent),0 0 18px var(--accent)}.frame-orbit{border-radius:50%;outline:2px dashed var(--accent);outline-offset:4px}.frame-paw{border-radius:48%;border:5px dotted var(--accent)}
.effect-glow .banner{filter:saturate(1.25) brightness(1.08)}.effect-aurora .banner,.effect-stars .banner{position:relative}.effect-aurora .banner:after{content:"";position:absolute;inset:0;background:linear-gradient(120deg,#35bdb155,#8d52bc55,#d46e8055);mix-blend-mode:screen;pointer-events:none}.effect-stars .banner:after{content:"";position:absolute;inset:0;background:radial-gradient(circle at 20% 25%,#fff9 0 2px,transparent 3px),radial-gradient(circle at 68% 30%,#fff9 0 1px,transparent 2px),radial-gradient(circle at 84% 72%,#fff8 0 1px,transparent 2px);pointer-events:none}.effect-pawprints .banner{position:relative}.effect-pawprints .banner:after{content:"🐾";position:absolute;right:18px;bottom:12px;font-size:30px;opacity:.75;pointer-events:none}
h1{margin:10px 0 0;font-size:30px}.slug{color:#aeb4c9;font-family:ui-monospace,monospace}.headline{color:#c4c9d9}
.content{padding:0 28px 30px}section{border-top:1px solid #303449;padding:18px 0}h2{font-size:16px;color:var(--accent);margin:0 0 9px}p{white-space:pre-wrap;line-height:1.65;color:#c8ccda;margin:0;overflow-wrap:anywhere}
ul{list-style:none;padding:0;margin:0;display:grid;gap:8px}li{display:grid;grid-template-columns:1fr auto;gap:3px 12px;padding:12px;background:#24283a;border-radius:12px}li span{color:#aeb4c9;font-size:14px}li a{grid-column:2;grid-row:1/3;align-self:center;color:var(--accent);text-decoration:none}
.badges{display:flex;flex-wrap:wrap;gap:8px}.badge{padding:6px 10px;border:1px solid var(--accent);border-radius:20px;color:var(--accent);font-size:13px}
footer{padding:16px 28px;border-top:1px solid #303449;color:#858da6;font-size:12px}
@media(max-width:520px){body{padding:10px}.banner{height:130px}.identity,.content{padding-left:18px;padding-right:18px}h1{font-size:24px}}
</style></head>
<body><main class="theme-${theme} effect-${effect}">${bannerMarkup}<div class="identity">${avatarMarkup}<h1>${escapeHtml(data.display_name)}</h1>
<div class="slug">@${escapeHtml(data.slug)}</div><p class="headline">${escapeHtml(data.headline)}</p></div>
<div class="content">${sections}${widgets}${activity}
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
