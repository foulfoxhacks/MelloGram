export const PROFILE_SCHEMA_VERSION = 1;

const THEMES = new Set(["midnight", "ocean", "aurora", "ember", "rose", "mint"]);
const FRAMES = new Set(["none", "glow", "neon", "orbit", "paw"]);
const EFFECTS = new Set(["none", "aurora", "stars", "pawprints", "glow"]);
const VISIBILITIES = new Set(["public", "unlisted", "private"]);
const KINDS = new Set(["games", "music", "projects", "communities", "activity", "about", "custom_link"]);

const isObject = (value) => value !== null && typeof value === "object" && !Array.isArray(value);
const textLength = (value, max) => typeof value === "string" && [...value].length <= max;
const webUrl = (value) => {
  if (typeof value !== "string" || !value) return false;
  try {
    const parsed = new URL(value);
    return parsed.protocol === "https:" || parsed.protocol === "http:";
  } catch {
    return false;
  }
};

export function validateAndSanitizeProfile(input) {
  if (!isObject(input) || input.schema_version !== PROFILE_SCHEMA_VERSION) {
    throw new Error("Unsupported MelloID profile schema.");
  }
  const profile = structuredClone(input);
  if (typeof profile.slug !== "string" || !/^[a-z0-9-]{1,32}$/.test(profile.slug)) {
    throw new Error("Profile handle must be 1–32 lowercase letters, numbers, or hyphens.");
  }
  if (typeof profile.display_name !== "string" || !profile.display_name.trim() || !textLength(profile.display_name, 64)) {
    throw new Error("Display name must contain 1–64 characters.");
  }
  if (!textLength(profile.headline || "", 120)) throw new Error("Headline is too long.");
  if (!VISIBILITIES.has(profile.visibility)) throw new Error("Unsupported profile visibility.");
  if (!THEMES.has(profile.profile_theme)) throw new Error("Unsupported profile theme.");
  if (!FRAMES.has(profile.avatar_frame)) throw new Error("Unsupported avatar frame.");
  if (!EFFECTS.has(profile.profile_effect)) throw new Error("Unsupported profile effect.");
  if (!/^#(?:[0-9a-f]{3}|[0-9a-f]{6})$/i.test(profile.accent_color || "")) {
    throw new Error("Accent color must be a #RGB or #RRGGBB value.");
  }
  if (profile.avatar_url && !webUrl(profile.avatar_url)) throw new Error("Avatar URL must use HTTP or HTTPS.");
  if (profile.banner_url && !webUrl(profile.banner_url)) throw new Error("Banner URL must use HTTP or HTTPS.");

  if (!Array.isArray(profile.bio_sections) || profile.bio_sections.length > 12) {
    throw new Error("Profiles support up to 12 biography sections.");
  }
  for (const section of profile.bio_sections) {
    if (!isObject(section) || typeof section.visible !== "boolean" || !textLength(section.heading || "", 48) || !textLength(section.body || "", 2000)) {
      throw new Error("A biography section exceeds its content limits.");
    }
  }

  if (!Array.isArray(profile.connections) || profile.connections.length > 40) {
    throw new Error("Profiles support up to 40 connections.");
  }
  for (const connection of profile.connections) {
    if (!isObject(connection) || typeof connection.visible !== "boolean" || !textLength(connection.provider || "", 40)
        || !connection.provider.trim() || !textLength(connection.display_name || "", 80)) {
      throw new Error("A connection has invalid provider or display-name data.");
    }
    if (connection.profile_url && !webUrl(connection.profile_url)) {
      throw new Error("Connection links must use HTTP or HTTPS.");
    }
    // Public profile uploads are not proof of account ownership. Verification
    // metadata is granted only after a future server-side provider challenge.
    connection.verification_issuer = null;
    connection.verified_at = null;
  }

  if (!Array.isArray(profile.badges) || profile.badges.length > 100) {
    throw new Error("Profiles support up to 100 badges.");
  }
  profile.badges = profile.badges
    .filter((badge) => isObject(badge) && badge.category === "collectible" && badge.server_issued !== true)
    .slice(0, 24)
    .map((badge) => {
      if (!textLength(badge.title || "", 48) || !badge.title.trim() || !textLength(badge.description || "", 160)) {
        throw new Error("A collectible badge has invalid title or description.");
      }
      return { ...badge, issuer: "self", awarded_at: null, server_issued: false, category: "collectible" };
    });

  if (!Array.isArray(profile.widgets) || profile.widgets.length > 24) {
    throw new Error("Profiles support up to 24 widgets.");
  }
  for (const widget of profile.widgets) {
    if (!isObject(widget) || typeof widget.enabled !== "boolean" || !KINDS.has(widget.kind) || !textLength(widget.title || "", 48)
        || !widget.title.trim() || !Number.isInteger(widget.position) || widget.position < 0) {
      throw new Error("A profile widget has invalid metadata.");
    }
    if (!isObject(widget.config)) throw new Error("Widget configuration must be an object.");
    const items = widget.config.items;
    if (items !== undefined && (!Array.isArray(items) || items.length > 20
        || items.some((item) => !textLength(item, 120)))) {
      throw new Error("Widgets support up to 20 text items of 120 characters each.");
    }
    if (widget.config.url && !webUrl(widget.config.url)) {
      throw new Error("Widget links must use HTTP or HTTPS.");
    }
  }

  // Presence is intentionally local-only until the hosted lease service is
  // implemented. Never publish a stale activity copied from a desktop backup.
  profile.presence = null;
  profile.show_presence = Boolean(profile.show_presence);
  delete profile.local_id;
  return profile;
}

export function projectVerifiedConnection(record) {
  if (!record || !["github", "discord"].includes(record.provider)
      || typeof record.provider_user_id !== "string"
      || typeof record.username !== "string"
      || typeof record.verified_at !== "string"
      || !record.profile_url || !webUrl(record.profile_url)
      || !record.visible) return null;
  const isGithub = record.provider === "github";
  return {
    id: `oauth-${record.provider}`,
    provider: isGithub ? "GitHub" : "Discord",
    kind: isGithub ? "development" : "social",
    display_name: record.display_name || record.username,
    profile_url: record.profile_url,
    avatar_url: record.avatar_url || null,
    verification_issuer: `${record.provider}-oauth`,
    verified_at: record.verified_at,
    visible: true,
  };
}

export function publicProfileProjection(profile, verifiedConnections = []) {
  if (!profile || profile.visibility === "private") return null;
  const output = {
    schema_version: profile.schema_version,
    slug: profile.slug,
    display_name: profile.display_name,
    headline: profile.headline,
    avatar_url: profile.avatar_url,
    banner_url: profile.banner_url,
    accent_color: profile.accent_color,
    profile_theme: profile.profile_theme,
    avatar_frame: profile.avatar_frame,
    profile_effect: profile.profile_effect,
    visibility: profile.visibility,
    bio_sections: (profile.bio_sections || []).filter((item) => item.visible),
    connections: [
      ...(profile.connections || []).filter((item) => item.visible
        && !verifiedConnections.some((trusted) => trusted.provider === String(item.provider || "").toLowerCase())),
      ...verifiedConnections.map(projectVerifiedConnection).filter(Boolean),
    ],
    badges: (profile.badges || []).filter((item) => item.visible),
    widgets: (profile.widgets || []).filter((item) => item.enabled).sort((a, b) => a.position - b.position),
    show_presence: false,
    presence: null,
  };
  return output;
}
