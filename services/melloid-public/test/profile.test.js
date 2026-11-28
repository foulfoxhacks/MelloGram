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

test("worker: HTTPS redirect, HSTS and unconfigured sign-in", async () => {
  const { default: worker } = await import("../src/worker.js");
  const env = { PUBLIC_BASE_URL: "https://id.example.com", ASSETS: { fetch: async () => new Response("<!doctype html>", { headers: { "Content-Type": "text/html" } }) }, DB: null };
  const redirect = await worker.fetch(new Request("http://id.example.com/@/someone"), env);
  assert.equal(redirect.status, 308);
  assert.equal(redirect.headers.get("location"), "https://id.example.com/@/someone");
  const login = await worker.fetch(new Request("https://id.example.com/auth/telegram/start"), env);
  assert.equal(login.status, 503);
  const page = await worker.fetch(new Request("https://id.example.com/"), env);
  assert.match(page.headers.get("strict-transport-security"), /max-age=/);
});

test("pages are fetched from their exact asset path, never a self-redirect", async () => {
  // Workers Static Assets answers /dashboard.html with a 307 to /dashboard when
  // extension handling is on, and /dashboard routes straight back here.
  const { default: worker } = await import("../src/worker.js");
  const seen = [];
  const env = {
    PUBLIC_BASE_URL: "https://id.example.com",
    DB: null,
    ASSETS: { fetch: async (request) => { seen.push(new URL(request.url).pathname); return new Response("<!doctype html>", { headers: { "Content-Type": "text/html" } }); } },
  };
  for (const [path, asset] of [["/", "/index.html"], ["/dashboard", "/dashboard.html"], ["/@/someone", "/profile.html"]]) {
    const response = await worker.fetch(new Request("https://id.example.com" + path), env);
    assert.equal(response.status, 200);
    assert.equal(seen.at(-1), asset, `GET ${path}`);
  }
});

test("badge lookup ids are validated, de-duplicated and capped", async () => {
  const { parseBadgeIds } = await import("../src/worker.js");
  assert.deepEqual(parseBadgeIds("12, 12,abc,0,-5,99999999999999999,34"), ["12", "34"]);
  assert.equal(parseBadgeIds(Array.from({ length: 80 }, (_, i) => i + 1).join(",")).length, 50);
  assert.deepEqual(parseBadgeIds(""), []);
});

const WIDGET_TOKEN = "123456:TEST-token";
const WIDGET_NOW = 1_800_000_000;
async function signWidget(fields, token = WIDGET_TOKEN) {
  const lines = Object.keys(fields).sort().map((k) => `${k}=${fields[k]}`).join("\n");
  const secret = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(token));
  const key = await crypto.subtle.importKey("raw", secret, { name: "HMAC", hash: "SHA-256" }, false, ["sign"]);
  const mac = new Uint8Array(await crypto.subtle.sign("HMAC", key, new TextEncoder().encode(lines)));
  return new URLSearchParams({ ...fields, hash: [...mac].map((b) => b.toString(16).padStart(2, "0")).join("") });
}
const widgetFields = () => ({ id: "42", first_name: "Ava", username: "ava", auth_date: String(WIDGET_NOW - 30) });

test("Telegram Login Widget hashes are verified and fresh", async () => {
  const { verifyTelegramWidget } = await import("../src/worker.js");
  const fields = widgetFields();
  assert.equal(await verifyTelegramWidget(await signWidget(fields), WIDGET_TOKEN, WIDGET_NOW), "42");
  const tampered = await signWidget(fields); tampered.set("id", "43");
  assert.equal(await verifyTelegramWidget(tampered, WIDGET_TOKEN, WIDGET_NOW), null);
  assert.equal(await verifyTelegramWidget(await signWidget({ ...fields, auth_date: String(WIDGET_NOW - 3600) }), WIDGET_TOKEN, WIDGET_NOW), null);
  assert.equal(await verifyTelegramWidget(await signWidget(fields), "other:token", WIDGET_NOW), null);
});

test("a signed field this build does not know still signs in", async () => {
  // Telegram signs every field it sends; refusing a signed newcomer (is_premium
  // today, something else tomorrow) would fail the whole sign-in.
  const { verifyTelegramWidgetPayload } = await import("../src/worker.js");
  const payload = await signWidget({ ...widgetFields(), is_premium: "true" });
  assert.deepEqual(await verifyTelegramWidgetPayload(payload, WIDGET_TOKEN, WIDGET_NOW), { id: "42", reason: null });
  const unsigned = await signWidget(widgetFields());
  unsigned.set("is_premium", "true");
  assert.deepEqual(await verifyTelegramWidgetPayload(unsigned, WIDGET_TOKEN, WIDGET_NOW), { id: null, reason: "signature" });
});

test("a bot token with a trailing newline or space still verifies", async () => {
  const { verifyTelegramWidgetPayload, widgetBotToken } = await import("../src/worker.js");
  const trimmed = widgetBotToken({ TELEGRAM_BOT_TOKEN: `  ${WIDGET_TOKEN}\n` });
  assert.equal(trimmed, WIDGET_TOKEN);
  const payload = await signWidget(widgetFields());
  assert.equal((await verifyTelegramWidgetPayload(payload, trimmed, WIDGET_NOW)).id, "42");
  assert.equal(widgetBotToken({ TELEGRAM_BOT_TOKEN: "   " }), null);
});

test("widget failures say which one happened", async () => {
  const { verifyTelegramWidgetPayload } = await import("../src/worker.js");
  assert.deepEqual(await verifyTelegramWidgetPayload(new URLSearchParams(), WIDGET_TOKEN, WIDGET_NOW), { id: null, reason: "missing" });
  const stale = await signWidget({ ...widgetFields(), auth_date: String(WIDGET_NOW - 3600) });
  assert.deepEqual(await verifyTelegramWidgetPayload(stale, WIDGET_TOKEN, WIDGET_NOW), { id: null, reason: "expired" });
  const wrongBot = await signWidget(widgetFields(), "987654:OTHER-bot");
  assert.deepEqual(await verifyTelegramWidgetPayload(wrongBot, WIDGET_TOKEN, WIDGET_NOW), { id: null, reason: "signature" });
  const duplicated = await signWidget(widgetFields());
  const doubled = new URLSearchParams(`${duplicated.toString()}&id=43`);
  assert.deepEqual(await verifyTelegramWidgetPayload(doubled, WIDGET_TOKEN, WIDGET_NOW), { id: null, reason: "duplicate_field" });
  const noHash = new URLSearchParams({ id: "42", auth_date: String(WIDGET_NOW) });
  assert.deepEqual(await verifyTelegramWidgetPayload(noHash, WIDGET_TOKEN, WIDGET_NOW), { id: null, reason: "malformed_hash" });
});

test("sign-in routes: widget when configured, 503 when nothing is", async () => {
  const { default: worker } = await import("../src/worker.js");
  const base = { PUBLIC_BASE_URL: "https://id.example.com", ASSETS: { fetch: async () => new Response("") }, DB: null };
  const none = await worker.fetch(new Request("https://id.example.com/auth/telegram/start"), base);
  assert.equal(none.status, 503);
  const env = { ...base, TELEGRAM_BOT_TOKEN: "1:x", TELEGRAM_BOT_USERNAME: "MelloIDBot" };
  const start = await worker.fetch(new Request("https://id.example.com/auth/telegram/start"), env);
  assert.equal(start.status, 303);
  assert.equal(start.headers.get("location"), "https://id.example.com/login");
  const login = await worker.fetch(new Request("https://id.example.com/login"), env);
  const html = await login.text();
  assert.match(html, /data-telegram-login="MelloIDBot"/);
  assert.match(login.headers.get("content-security-policy"), /script-src https:\/\/telegram\.org/);
});

test("a missing bot token is a configuration problem, not a bad signature", async () => {
  const { verifyTelegramWidgetPayload } = await import("../src/worker.js");
  const payload = await signWidget(widgetFields());
  assert.deepEqual(await verifyTelegramWidgetPayload(payload, null, WIDGET_NOW), { id: null, reason: "bot_token_missing" });
  assert.deepEqual(await verifyTelegramWidgetPayload(payload, "", WIDGET_NOW), { id: null, reason: "bot_token_missing" });
});

test("a signature failure does not claim the bot is wrong when only its token suffix differs", async () => {
  const { default: worker, botIdOf } = await import("../src/worker.js");
  assert.equal(botIdOf("8413521846:AAH-secret"), "8413521846");
  assert.equal(botIdOf("not-a-token"), null);
  assert.equal(botIdOf(null), null);

  // The route verifies against the wall clock, so the payload must be fresh.
  const payload = await signWidget({ ...widgetFields(), auth_date: String(Math.floor(Date.now() / 1000)) }, "987654321:AAH-current");
  const env = {
    PUBLIC_BASE_URL: "https://id.example.com",
    DB: null,
    TELEGRAM_BOT_TOKEN: "987654321:AAH-stale",
    TELEGRAM_BOT_USERNAME: "MelloIDBot",
  };
  const { url, cookie } = await widgetAttempt(worker, env);
  url.search = payload.toString();
  const res = await worker.fetch(new Request(url, { headers: { Cookie: cookie } }), env);
  assert.equal(res.status, 401);
  const html = await res.text();
  assert.match(html, /Reference: <code>signature<\/code>/);
  assert.match(html, /Start a fresh sign-in from MelloID/);
  assert.equal(res.headers.get("set-cookie"), null);
  assert.doesNotMatch(html, /different bot|belongs to|AAH-stale/);
});

async function widgetAttempt(worker, env) {
  const login = await worker.fetch(new Request(new URL("/login", env.PUBLIC_BASE_URL)), env);
  const cookie = login.headers.get("set-cookie").split(";")[0];
  const html = await login.text();
  return { url: new URL(html.match(/data-auth-url="([^"]+)"/)[1]), cookie };
}

test("Telegram profile id is distinct from OIDC subject and rejects imprecise values", async () => {
  const { telegramProfileId } = await import("../src/worker.js");
  assert.equal(telegramProfileId(987654321), "987654321");
  assert.equal(telegramProfileId("987654321"), "987654321");
  for (const invalid of [null, -1, 0, "0", "01", "1e3", 2.5, 9007199254740992, "1234123412341234123"]) {
    assert.equal(telegramProfileId(invalid), null);
  }
});

test("widget confirmation requires browser state and creates exactly one session", async (t) => {
  const { default: worker } = await import("../src/worker.js");
  const sqlite = await import("node:sqlite").catch((error) => {
    if (error.code !== "ERR_UNKNOWN_BUILTIN_MODULE") throw error;
    return null;
  });
  if (!sqlite) return t.skip("The SQL integration test requires Node 22 with node:sqlite");
  const { DatabaseSync } = sqlite;
  const { readFileSync } = await import("node:fs");
  const db = new DatabaseSync(":memory:");
  try {
    for (const name of ["0001_init.sql", "0003_widget_confirmations.sql"]) {
      db.exec(readFileSync(new URL(`../migrations/${name}`, import.meta.url), "utf8"));
    }
    // D1's numbered placeholders map to SQLite's named parameter binding.
    const DB = {
      prepare: (sql) => ({ bind: (...values) => () => {
        const bindings = Object.fromEntries(values.map((value, i) => [`?${i + 1}`, value]));
        const { changes } = db.prepare(sql).run(bindings);
        return { meta: { changes } };
      } }),
      batch: async (statements) => {
        db.exec("BEGIN");
        try {
          const results = statements.map((statement) => statement());
          db.exec("COMMIT");
          return results;
        } catch (error) {
          db.exec("ROLLBACK");
          throw error;
        }
      },
    };
    const env = { PUBLIC_BASE_URL: "https://id.example.com", TELEGRAM_BOT_TOKEN: WIDGET_TOKEN, TELEGRAM_BOT_USERNAME: "MelloIDBot", DB };
    const first = await widgetAttempt(worker, env);
    const other = await widgetAttempt(worker, env);
    const payload = await signWidget({ ...widgetFields(), auth_date: String(Math.floor(Date.now() / 1000)), future_field: "unchanged" });
    first.url.search = other.url.search = payload.toString();
    const callback = (url, cookie) => worker.fetch(new Request(url, { headers: cookie ? { Cookie: cookie } : {} }), env);
    for (const rejected of [await callback(first.url), await callback(first.url, other.cookie)]) {
      assert.equal(rejected.status, 400);
      assert.equal(rejected.headers.get("set-cookie"), null);
    }
    const success = await callback(first.url, first.cookie);
    assert.equal(success.status, 303, await success.text());
    assert.equal(success.headers.get("location"), "https://id.example.com/dashboard");
    assert.equal(success.headers.get("referrer-policy"), "no-referrer");
    assert.match(success.headers.get("set-cookie"), /melloid_session=/);
    const duplicate = await callback(other.url, other.cookie);
    const secondConfirmation = await signWidget({ ...widgetFields(), auth_date: String(Math.floor(Date.now() / 1000)), first_name: "Different confirmation" });
    first.url.search = secondConfirmation.toString();
    const reusedState = await callback(first.url, first.cookie);
    assert.equal(reusedState.status, 401);
    assert.match(await reusedState.text(), /Reference: <code>used<\/code>/);
    assert.equal(duplicate.status, 401);
    assert.match(await duplicate.text(), /Reference: <code>used<\/code>/);
    assert.equal(db.prepare("SELECT count(*) AS count FROM sessions WHERE owner_telegram_id = '42'").get().count, 1);
  } finally {
    db.close();
  }
});

test("OIDC stores Telegram's verified profile id rather than the OIDC subject", async (t) => {
  const { default: worker } = await import("../src/worker.js");
  const { generateKeyPair, exportJWK, SignJWT } = await import("jose");
  const { privateKey, publicKey } = await generateKeyPair("RS256");
  const env = {
    PUBLIC_BASE_URL: "https://id.example.com",
    TELEGRAM_CLIENT_ID: "123456",
    TELEGRAM_CLIENT_SECRET: "synthetic",
    DB: { prepare: () => ({ bind: (...values) => ({ run: async () => { stored = values; } }) }) },
  };
  let stored;
  const start = await worker.fetch(new Request(`${env.PUBLIC_BASE_URL}/auth/telegram/start`), env);
  assert.equal(start.status, 303);
  const redirect = new URL(start.headers.get("location"));
  const { searchParams } = redirect;
  const state = searchParams.get("state");
  const nonce = searchParams.get("nonce");
  const cookie = start.headers.getSetCookie().map((part) => part.split(";")[0]).join("; ");
  const token = await new SignJWT({ id: 987654321, nonce })
    .setProtectedHeader({ alg: "RS256", kid: "test" })
    .setSubject("1234123412341234123")
    .setIssuer("https://oauth.telegram.org").setAudience(env.TELEGRAM_CLIENT_ID)
    .setExpirationTime("5m").sign(privateKey);
  const jwk = { ...await exportJWK(publicKey), alg: "RS256", kid: "test" };
  t.mock.method(globalThis, "fetch", async (input) => {
    switch (String(input)) {
      case "https://oauth.telegram.org/token": return Response.json({ id_token: token });
      case "https://oauth.telegram.org/.well-known/jwks.json": return Response.json({ keys: [jwk] });
      default: throw new Error("Unexpected upstream call");
    }
  });
  const response = await worker.fetch(new Request(
    `${env.PUBLIC_BASE_URL}/auth/telegram/callback?state=${state}&code=fixture`, { headers: { Cookie: cookie } },
  ), env);
  assert.equal(response.status, 303, await response.text());
  assert.equal(stored[1], "987654321");
});
