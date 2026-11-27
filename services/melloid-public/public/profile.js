"use strict";

const root = document.getElementById("profile-root");
const status = document.getElementById("profile-status");
const allowedThemes = new Set(["midnight", "ocean", "aurora", "ember", "rose", "mint"]);
const allowedFrames = new Set(["none", "glow", "neon", "orbit", "paw"]);
const allowedEffects = new Set(["none", "aurora", "stars", "pawprints", "glow"]);
const safeUrl = (value) => {
  try {
    const url = new URL(value);
    return ["https:", "http:"].includes(url.protocol) ? url.href : null;
  } catch { return null; }
};
function el(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined && text !== null) node.textContent = String(text);
  return node;
}
function section(title) {
  const node = el("section", "profile-section");
  node.append(el("h2", "", title));
  return node;
}
function render(profile) {
  const theme = allowedThemes.has(profile.profile_theme) ? profile.profile_theme : "midnight";
  const frame = allowedFrames.has(profile.avatar_frame) ? profile.avatar_frame : "none";
  const effect = allowedEffects.has(profile.profile_effect) ? profile.profile_effect : "none";
  const card = el("article", `public-profile theme-${theme} effect-${effect}`);
  card.style.setProperty("--accent", /^#(?:[0-9a-f]{3}|[0-9a-f]{6})$/i.test(profile.accent_color || "") ? profile.accent_color : "#35bdb1");
  const banner = el("div", "profile-banner");
  const bannerUrl = safeUrl(profile.banner_url);
  if (bannerUrl) {
    const image = el("img");
    image.src = bannerUrl;
    image.alt = "Profile banner";
    banner.append(image);
  }
  const head = el("div", "profile-head");
  const avatarUrl = safeUrl(profile.avatar_url);
  const avatar = avatarUrl ? el("img", `profile-avatar frame-${frame}`) : el("div", `profile-avatar initials frame-${frame}`, (profile.display_name || "M").split(/\s+/).slice(0, 2).map((x) => x[0] || "").join("").toUpperCase());
  if (avatarUrl) { avatar.src = avatarUrl; avatar.alt = "Profile avatar"; }
  head.append(avatar, el("h1", "profile-name", profile.display_name), el("div", "profile-slug", "@" + profile.slug));
  if (profile.headline) head.append(el("p", "profile-headline", profile.headline));
  const content = el("div", "profile-content");

  for (const bio of profile.bio_sections || []) {
    const part = section(bio.heading || "About");
    part.append(el("p", "", bio.body));
    content.append(part);
  }
  for (const widget of profile.widgets || []) {
    const part = section(widget.title || "Widget");
    const items = Array.isArray(widget.config?.items) ? widget.config.items.filter((x) => typeof x === "string") : [];
    if (items.length) {
      const list = el("div", "widget-items");
      for (const item of items) list.append(el("span", "widget-item", item));
      part.append(list);
    }
    const link = safeUrl(widget.config?.url);
    if (link) {
      const anchor = el("a", "", "Open link ↗");
      anchor.href = link;
      anchor.target = "_blank";
      anchor.rel = "noopener noreferrer";
      part.append(anchor);
    }
    if (!items.length && !link) part.append(el("p", "", "No content added."));
    content.append(part);
  }
  const connections = section("Connections");
  if ((profile.connections || []).length) {
    for (const connection of profile.connections) {
      const item = el("div", "connection");
      item.append(el("strong", "", connection.provider), el("span", "", connection.display_name));
      const link = safeUrl(connection.profile_url);
      if (link) {
        const anchor = el("a", "", "Open ↗");
        anchor.href = link;
        anchor.target = "_blank";
        anchor.rel = "noopener noreferrer";
        item.append(anchor);
      }
      connections.append(item);
    }
  } else connections.append(el("p", "", "No public connections."));
  content.append(connections);

  const badges = section("Badges");
  if ((profile.badges || []).length) {
    const list = el("div", "badges");
    for (const badge of profile.badges) list.append(el("span", "badge", "✦ " + badge.title));
    badges.append(list);
  } else badges.append(el("p", "", "No public collectible badges."));
  content.append(badges);
  card.append(banner, head, content);
  root.replaceChildren(card);
  root.hidden = false;
  status.hidden = true;
  document.title = `${profile.display_name} (@${profile.slug}) · MelloID`;
}
async function load() {
  const slug = location.pathname.split("/").filter(Boolean).at(-1) || "";
  if (!/^[a-z0-9-]{1,32}$/.test(slug)) {
    status.textContent = "This MelloID profile was not found.";
    return;
  }
  try {
    const response = await fetch("/api/v1/profiles/" + encodeURIComponent(slug));
    const body = await response.json().catch(() => ({}));
    if (!response.ok) throw new Error(body.error || "Profile not found.");
    render(body.profile);
  } catch (error) {
    status.textContent = error.message;
  }
}
load();
