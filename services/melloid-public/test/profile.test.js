import test from "node:test";
import assert from "node:assert/strict";
import { projectVerifiedConnection, publicProfileProjection, validateAndSanitizeProfile } from "../src/profile.js";
import { normalizeProviderIdentity, normalizeSteamIdentity, validateSteamOpenIdResponse } from "../src/connections.js";

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

test("GitHub identity normalization only accepts github.com profile URLs", () => {
  const identity = normalizeProviderIdentity("github", {
    id: 12345,
    login: "sammydev",
    name: "Sammy Dev",
    html_url: "https://github.com/sammydev",
    avatar_url: "https://avatars.githubusercontent.com/u/12345"
  });
  assert.equal(identity.provider_user_id, "12345");
  assert.equal(identity.username, "sammydev");
  assert.equal(identity.profile_url, "https://github.com/sammydev");
  assert.equal(identity.avatar_url, "https://avatars.githubusercontent.com/u/12345");
  assert.throws(() => normalizeProviderIdentity("github", {
    id: 12345, login: "sammydev", html_url: "https://evil.example/sammydev"
  }));
});

test("Discord identity normalization derives the profile URL from the stable account ID", () => {
  const identity = normalizeProviderIdentity("discord", {
    id: "123456789012345678",
    username: "mello",
    global_name: "Mello Fox",
    avatar: "a_123abc"
  });
  assert.equal(identity.profile_url, "https://discord.com/users/123456789012345678");
  assert.equal(identity.display_name, "Mello Fox");
  assert.equal(identity.avatar_url, "https://cdn.discordapp.com/avatars/123456789012345678/a_123abc.png?size=128");
  assert.throws(() => normalizeProviderIdentity("discord", { id: "bad", username: "mello" }));
});

test("Steam OpenID identity normalization accepts a SteamID64 and only Steam profile assets", () => {
  const identity = normalizeSteamIdentity("76561198000000000", {
    personaname: "Sammy",
    profileurl: "https://steamcommunity.com/profiles/76561198000000000",
    avatarfull: "https://avatars.steamstatic.com/avatar.jpg"
  });
  assert.equal(identity.provider_user_id, "76561198000000000");
  assert.equal(identity.display_name, "Sammy");
  assert.equal(identity.profile_url, "https://steamcommunity.com/profiles/76561198000000000");
  assert.equal(identity.avatar_url, "https://avatars.steamstatic.com/avatar.jpg");
  assert.throws(() => normalizeSteamIdentity("not-a-steamid"));
  const unsafe = normalizeSteamIdentity("76561198000000000", {
    profileurl: "https://evil.example/profile",
    avatarfull: "https://evil.example/avatar.png"
  });
  assert.equal(unsafe.profile_url, "https://steamcommunity.com/profiles/76561198000000000");
  assert.equal(unsafe.avatar_url, null);
});

test("Steam OpenID callback validation binds the exact return URL and signed SteamID64", () => {
  const now = 1_795_744_000;
  const nonce = new Date(now * 1000).toISOString().replace(".000", "") + "unique";
  const returnTo = "https://profiles.example.com/auth/connections/steam/callback?state=one-time-state";
  const params = new URLSearchParams({
    "openid.ns": "http://specs.openid.net/auth/2.0",
    "openid.mode": "id_res",
    "openid.op_endpoint": "https://steamcommunity.com/openid/login",
    "openid.claimed_id": "https://steamcommunity.com/openid/id/76561198000000000",
    "openid.identity": "https://steamcommunity.com/openid/id/76561198000000000",
    "openid.return_to": returnTo,
    "openid.response_nonce": nonce,
    "openid.assoc_handle": "steam-handle",
    "openid.signed": "op_endpoint,claimed_id,identity,return_to,response_nonce",
    "openid.sig": "signed-assertion"
  });
  assert.deepEqual(validateSteamOpenIdResponse(params, returnTo, now), {
    steamId: "76561198000000000",
    responseNonce: nonce,
    opEndpoint: "https://steamcommunity.com/openid/login"
  });
  assert.equal(validateSteamOpenIdResponse(params, returnTo + "&other=1", now), null);
  const stale = new URLSearchParams(params);
  stale.set("openid.response_nonce", "2020-01-01T00:00:00Zold");
  assert.equal(validateSteamOpenIdResponse(stale, returnTo, now), null);
  const unsigned = new URLSearchParams(params);
  unsigned.set("openid.signed", "claimed_id,identity,return_to,response_nonce");
  assert.equal(validateSteamOpenIdResponse(unsigned, returnTo, now), null);
});

test("Steam verified account projection is marked with the OpenID issuer", () => {
  const connection = projectVerifiedConnection({
    provider: "steam",
    provider_user_id: "76561198000000000",
    username: "76561198000000000",
    display_name: "Sammy",
    profile_url: "https://steamcommunity.com/profiles/76561198000000000",
    avatar_url: null,
    verified_at: "2026-11-27T00:00:00Z",
    visible: 1
  });
  assert.equal(connection.provider, "Steam");
  assert.equal(connection.kind, "gaming");
  assert.equal(connection.verification_issuer, "steam-openid");
});

test("Steam verified connection projection uses its OpenID issuer and gaming category", () => {
  const connection = projectVerifiedConnection({
    provider: "steam",
    provider_user_id: "76561198000000000",
    username: "76561198000000000",
    display_name: "Sammy",
    profile_url: "https://steamcommunity.com/profiles/76561198000000000",
    avatar_url: null,
    verified_at: "2026-11-27T00:00:00Z",
    visible: 1
  });
  assert.equal(connection.provider, "Steam");
  assert.equal(connection.kind, "gaming");
  assert.equal(connection.verification_issuer, "steam-openid");
});
