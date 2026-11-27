import test from "node:test";
import assert from "node:assert/strict";
import { publicProfileProjection, validateAndSanitizeProfile } from "../src/profile.js";

function sample() {
  return {
    schema_version: 1,
    local_id: "local-user",
    slug: "sample-user",
    display_name: "Sample User",
    headline: "Creator and gamer",
    avatar_url: null,
    banner_url: null,
    accent_color: "#35bdb1",
    profile_theme: "midnight",
    avatar_frame: "none",
    profile_effect: "none",
    visibility: "public",
    bio_sections: [
      { id: "about", heading: "About", body: "Hello", visible: true },
      { id: "private", heading: "Private note", body: "Do not show", visible: false }
    ],
    connections: [
      { id: "github", provider: "GitHub", display_name: "sample", profile_url: "https://github.com/sample", visible: true, verification_issuer: "forged", verified_at: "now" },
      { id: "secret", provider: "Steam", display_name: "hidden", profile_url: null, visible: false }
    ],
    badges: [
      { id: "founder", title: "Founder", description: "Self-picked", category: "collectible", issuer: "official", server_issued: false, visible: true },
      { id: "fake-verified", title: "Verified", description: "", category: "verified", issuer: "self", server_issued: true, visible: true }
    ],
    widgets: [
      { id: "games", kind: "games", title: "Games", enabled: true, position: 0, config: { items: ["Minecraft"] } },
      { id: "hidden", kind: "about", title: "Hidden", enabled: false, position: 1, config: { items: ["Secret"] } }
    ],
    show_presence: true,
    presence: { application: "Old Game", activity: "Stale activity" },
    created_at: "2026-01-01T00:00:00Z",
    updated_at: "2026-01-01T00:00:00Z"
  };
}

test("sanitization strips client-forged account verification and trusted badge categories", () => {
  const profile = validateAndSanitizeProfile(sample());
  assert.equal(profile.local_id, undefined);
  assert.equal(profile.connections[0].verification_issuer, null);
  assert.equal(profile.connections[0].verified_at, null);
  assert.equal(profile.badges.length, 1);
  assert.equal(profile.badges[0].issuer, "self");
  assert.equal(profile.badges[0].server_issued, false);
  assert.equal(profile.presence, null);
});

test("public projection filters hidden sections, connections and disabled widgets", () => {
  const profile = validateAndSanitizeProfile(sample());
  const output = publicProfileProjection(profile);
  assert.deepEqual(output.bio_sections.map((item) => item.id), ["about"]);
  assert.deepEqual(output.connections.map((item) => item.id), ["github"]);
  assert.deepEqual(output.widgets.map((item) => item.id), ["games"]);
  assert.equal(output.presence, null);
});

test("private profiles do not have a public projection", () => {
  const profile = validateAndSanitizeProfile({ ...sample(), visibility: "private" });
  assert.equal(publicProfileProjection(profile), null);
});

test("invalid handles and unsafe links are rejected", () => {
  assert.throws(() => validateAndSanitizeProfile({ ...sample(), slug: "../admin" }));
  const profile = sample();
  profile.connections[0].profile_url = "javascript:alert(1)";
  assert.throws(() => validateAndSanitizeProfile(profile));
});

test("widget item and appearance allowlists are enforced", () => {
  const invalidWidget = sample();
  invalidWidget.widgets[0].config.items = ["x".repeat(121)];
  assert.throws(() => validateAndSanitizeProfile(invalidWidget));
  const invalidTheme = sample();
  invalidTheme.profile_theme = "body{background:red}";
  assert.throws(() => validateAndSanitizeProfile(invalidTheme));
});

test("visibility flags must be real booleans to prevent hidden-field leakage", () => {
  const profile = sample();
  profile.bio_sections[1].visible = "false";
  assert.throws(() => validateAndSanitizeProfile(profile));
  const widgetProfile = sample();
  widgetProfile.widgets[0].enabled = "false";
  assert.throws(() => validateAndSanitizeProfile(widgetProfile));
});
