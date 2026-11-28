import { createRemoteJWKSet, jwtVerify } from "jose";
import { publicProfileProjection, validateAndSanitizeProfile } from "./profile.js";
import { normalizeProviderIdentity, normalizeSteamIdentity, validateSteamOpenIdResponse } from "./connections.js";

const TELEGRAM_ISSUER = "https://oauth.telegram.org";
const TELEGRAM_JWKS = createRemoteJWKSet(new URL("https://oauth.telegram.org/.well-known/jwks.json"));
const SESSION_TTL_SECONDS = 60 * 60 * 24 * 14;
const MAX_PROFILE_BYTES = 256 * 1024;

const encoder = new TextEncoder();
const randomToken = (bytes = 32) => {
  const data = crypto.getRandomValues(new Uint8Array(bytes));
  return base64url(data);
};
function base64url(data) {
  const bytes = data instanceof ArrayBuffer ? new Uint8Array(data) : data;
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary).replace(/=/g, "").replace(/\+/g, "-").replace(/\//g, "_");
}
async function sha256(value) {
  const digest = await crypto.subtle.digest("SHA-256", encoder.encode(value));
  return [...new Uint8Array(digest)].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}
function cookie(request, name) {
  const header = request.headers.get("Cookie") || "";
  const item = header.split(";").map((part) => part.trim()).find((part) => part.startsWith(name + "="));
  return item ? decodeURIComponent(item.slice(name.length + 1)) : null;
}
function cookieHeader(name, value, maxAge) {
  return `${name}=${encodeURIComponent(value)}; Path=/; Max-Age=${maxAge}; HttpOnly; Secure; SameSite=Lax`;
}
function expiredCookie(name) {
  return cookieHeader(name, "", 0);
}
function json(data, status = 200, headers = {}) {
  return new Response(JSON.stringify(data), {
    status,
    headers: { "Content-Type": "application/json; charset=utf-8", "Cache-Control": "no-store", ...headers },
  });
}
function redirect(location, cookies = []) {
  const headers = new Headers({ Location: location, "Cache-Control": "no-store" });
  for (const value of cookies) headers.append("Set-Cookie", value);
  return new Response(null, { status: 303, headers });
}
const escapeHtml = (value) => String(value).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);
function errorPage(message, status = 400, reference = null) {
  // The reference is a fixed reason code (never request data), so the person
  // running the deployment can tell an expired confirmation from a bot token
  // that does not match the bot, without reading Worker logs.
  const note = reference ? `<p class="muted">Reference: <code>${escapeHtml(reference)}</code></p>` : "";
  return new Response(`<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>MelloID sign-in error</title><link rel="stylesheet" href="/app.css"></head><body><main class="shell tg-shell"><section class="tg-hero-id"><img class="tg-hero-logo" src="/mark.png" alt="" width="132" height="132"><h1>Sign-in failed</h1><p class="lead">${escapeHtml(message)}</p>${note}<p class="muted"><a href="/">Return home</a></p></section></main></body></html>`, {
    status,
    headers: { "Content-Type": "text/html; charset=utf-8", "Cache-Control": "no-store" },
  });
}
function requireSameOrigin(request, env) {
  const origin = request.headers.get("Origin");
  return origin === new URL(env.PUBLIC_BASE_URL).origin;
}
function setSecurityHeaders(response) {
  const headers = new Headers(response.headers);
  headers.set("X-Content-Type-Options", "nosniff");
  headers.set("Referrer-Policy", "strict-origin-when-cross-origin");
  headers.set("X-Frame-Options", "DENY");
  headers.set("Permissions-Policy", "camera=(), microphone=(), geolocation=()");
  headers.set("Strict-Transport-Security", "max-age=63072000");
  // A page that sets its own (narrower or Telegram-widget) CSP keeps it.
  if (headers.get("Content-Type")?.includes("text/html") && !headers.has("Content-Security-Policy")) {
    headers.set("Content-Security-Policy", "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' https: data:; connect-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'");
  }
  return new Response(response.body, { status: response.status, statusText: response.statusText, headers });
}
function requireConfigured(env) {
  for (const key of ["PUBLIC_BASE_URL", "TELEGRAM_CLIENT_ID", "TELEGRAM_CLIENT_SECRET"]) {
    if (!env[key]) throw new Error(`Missing Worker configuration: ${key}`);
  }
}
async function getSession(request, env) {
  const sessionId = cookie(request, "melloid_session");
  if (!sessionId || sessionId.length > 128) return null;
  const sessionHash = await sha256(sessionId);
  const row = await env.DB.prepare(
    "SELECT owner_telegram_id, expires_at FROM sessions WHERE session_hash = ?1"
  ).bind(sessionHash).first();
  if (!row || row.expires_at <= Math.floor(Date.now() / 1000)) {
    if (row) await env.DB.prepare("DELETE FROM sessions WHERE session_hash = ?1").bind(sessionHash).run();
    return null;
  }
  return { telegramId: row.owner_telegram_id, sessionHash };
}
async function requireSession(request, env) {
  const session = await getSession(request, env);
  return session || null;
}
async function startTelegramLogin(request, env) {
  requireConfigured(env);
  const state = randomToken();
  const nonce = randomToken();
  const verifier = randomToken(48);
  const challenge = base64url(await crypto.subtle.digest("SHA-256", encoder.encode(verifier)));
  const callback = new URL("/auth/telegram/callback", env.PUBLIC_BASE_URL).toString();
  const authorize = new URL("/auth", TELEGRAM_ISSUER);
  authorize.searchParams.set("client_id", env.TELEGRAM_CLIENT_ID);
  authorize.searchParams.set("redirect_uri", callback);
  authorize.searchParams.set("response_type", "code");
  authorize.searchParams.set("scope", "openid profile");
  authorize.searchParams.set("state", state);
  authorize.searchParams.set("nonce", nonce);
  authorize.searchParams.set("code_challenge", challenge);
  authorize.searchParams.set("code_challenge_method", "S256");
  return redirect(authorize.toString(), [
    cookieHeader("melloid_oauth_state", state, 600),
    cookieHeader("melloid_oauth_nonce", nonce, 600),
    cookieHeader("melloid_pkce_verifier", verifier, 600),
  ]);
}
async function finishTelegramLogin(request, env) {
  requireConfigured(env);
  const url = new URL(request.url);
  const state = url.searchParams.get("state");
  const code = url.searchParams.get("code");
  const expectedState = cookie(request, "melloid_oauth_state");
  const nonce = cookie(request, "melloid_oauth_nonce");
  const verifier = cookie(request, "melloid_pkce_verifier");
  const clearOAuth = [
    expiredCookie("melloid_oauth_state"),
    expiredCookie("melloid_oauth_nonce"),
    expiredCookie("melloid_pkce_verifier"),
  ];
  if (!state || state !== expectedState || !nonce || !verifier || !code) {
    return errorPage("The authorization state expired or did not match. Please sign in again.", 400);
  }
  const callback = new URL("/auth/telegram/callback", env.PUBLIC_BASE_URL).toString();
  const response = await fetch(new URL("/token", TELEGRAM_ISSUER), {
    method: "POST",
    headers: {
      "Authorization": "Basic " + btoa(`${env.TELEGRAM_CLIENT_ID}:${env.TELEGRAM_CLIENT_SECRET}`),
      "Content-Type": "application/x-www-form-urlencoded",
    },
    body: new URLSearchParams({
      grant_type: "authorization_code",
      code,
      redirect_uri: callback,
      client_id: env.TELEGRAM_CLIENT_ID,
      code_verifier: verifier,
    }),
  });
  if (!response.ok) return errorPage("Telegram did not accept the authorization code.", 401);
  const tokens = await response.json();
  if (typeof tokens.id_token !== "string") return errorPage("Telegram did not return an ID token.", 401);
  let payload;
  try {
    ({ payload } = await jwtVerify(tokens.id_token, TELEGRAM_JWKS, {
      issuer: TELEGRAM_ISSUER,
      audience: env.TELEGRAM_CLIENT_ID,
      algorithms: ["RS256"],
    }));
  } catch {
    return errorPage("The Telegram identity token could not be verified.", 401);
  }
  if (payload.nonce !== nonce || typeof payload.sub !== "string" || !/^\d{1,24}$/.test(payload.sub)) {
    return errorPage("The Telegram identity token did not match this sign-in request.", 401);
  }

  const sessionId = randomToken();
  const expiresAt = Math.floor(Date.now() / 1000) + SESSION_TTL_SECONDS;
  await env.DB.prepare(
    "INSERT INTO sessions (session_hash, owner_telegram_id, expires_at, created_at) VALUES (?1, ?2, ?3, ?4)"
  ).bind(await sha256(sessionId), payload.sub, expiresAt, Math.floor(Date.now() / 1000)).run();
  return redirect(new URL("/dashboard", env.PUBLIC_BASE_URL).toString(), [
    ...clearOAuth,
    cookieHeader("melloid_session", sessionId, SESSION_TTL_SECONDS),
  ]);
}
// ---- Telegram Login Widget -------------------------------------------------------------
//
// The widget (telegram.org/js/telegram-widget.js) needs only a bot from
// @BotFather with /setdomain pointing at this host. Telegram redirects back
// with the user's id, names, username, photo_url, auth_date and a hash; the
// hash is HMAC-SHA256 over the sorted "key=value" lines, keyed with
// SHA-256(bot token). Nothing else (no phone number, no session) is involved.

const WIDGET_MAX_AGE_SECONDS = 600;
const BOT_USERNAME = /^[A-Za-z0-9_]{5,32}$/;

/**
 * The configured bot token, trimmed. A secret pasted into a dashboard keeps
 * any trailing newline or space, and that one character silently breaks every
 * Login Widget signature.
 */
export function widgetBotToken(env) {
  const token = typeof env?.TELEGRAM_BOT_TOKEN === "string" ? env.TELEGRAM_BOT_TOKEN.trim() : "";
  return token || null;
}
/**
 * The bot id half of a bot token (`8413521846:AAH…`). Bot ids are public —
 * they appear in every Bot API URL — so an error page may name one; the secret
 * half never leaves this function.
 */
export function botIdOf(token) {
  const id = String(token || "").split(":")[0];
  return /^[0-9]{5,12}$/.test(id) ? id : null;
}
export function widgetConfigured(env) {
  return Boolean(widgetBotToken(env) && BOT_USERNAME.test(env.TELEGRAM_BOT_USERNAME || ""));
}
export function oidcConfigured(env) {
  return Boolean(env.TELEGRAM_CLIENT_ID && env.TELEGRAM_CLIENT_SECRET);
}

/**
 * Verifies the payload `data-auth-url` receives and reports why it was not
 * accepted. Returns `{ id, reason }`: `id` is the verified Telegram user id,
 * or null with a short reason code ("missing", "malformed_hash", "expired",
 * "signature", ...).
 *
 * The signature covers every field Telegram sent except `hash`, exactly as
 * telegram.org documents it, so fields this build has never heard of are
 * included rather than rejected: an unsigned extra parameter can only make the
 * HMAC fail, and refusing a *signed* field that Telegram starts sending would
 * break sign-in for everyone.
 */
export async function verifyTelegramWidgetPayload(params, botToken, now = Math.floor(Date.now() / 1000)) {
  // Without a token there is nothing to verify against. Reporting that as a
  // bad signature sends the operator looking at the wrong bot.
  if (!botToken) return { id: null, reason: "bot_token_missing" };
  const names = [...params.keys()];
  if (!names.length) return { id: null, reason: "missing" };
  if (new Set(names).size !== names.length) return { id: null, reason: "duplicate_field" };
  const hash = params.get("hash") || "";
  if (!/^[0-9a-f]{64}$/.test(hash)) return { id: null, reason: "malformed_hash" };
  const id = params.get("id") || "";
  if (!/^[1-9][0-9]{0,15}$/.test(id)) return { id: null, reason: "malformed_id" };
  const authDate = Number(params.get("auth_date"));
  if (!Number.isInteger(authDate)) return { id: null, reason: "malformed_auth_date" };
  if (authDate > now + 60 || now - authDate > WIDGET_MAX_AGE_SECONDS) return { id: null, reason: "expired" };
  const lines = names
    .filter((key) => key !== "hash" && key !== "")
    .sort()
    .map((key) => `${key}=${params.get(key)}`);
  const secret = await crypto.subtle.digest("SHA-256", encoder.encode(botToken));
  const key = await crypto.subtle.importKey("raw", secret, { name: "HMAC", hash: "SHA-256" }, false, ["sign"]);
  const mac = new Uint8Array(await crypto.subtle.sign("HMAC", key, encoder.encode(lines.join("\n"))));
  const expected = [...mac].map((b) => b.toString(16).padStart(2, "0")).join("");
  let diff = 0;
  for (let i = 0; i < 64; i++) diff |= expected.charCodeAt(i) ^ hash.charCodeAt(i);
  return diff === 0 ? { id, reason: null } : { id: null, reason: "signature" };
}

/** Returns the verified Telegram user id, or null. */
export async function verifyTelegramWidget(params, botToken, now = Math.floor(Date.now() / 1000)) {
  const { id } = await verifyTelegramWidgetPayload(params, botToken, now);
  return id;
}

function widgetLoginPage(env) {
  const bot = env.TELEGRAM_BOT_USERNAME;
  const authUrl = new URL("/auth/telegram/widget", env.PUBLIC_BASE_URL).toString();
  const html = `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>Sign in to MelloID</title><link rel="stylesheet" href="/app.css"></head>
<body><main class="shell tg-shell"><section class="tg-hero-id">
<img class="tg-hero-logo" src="/mark.png" alt="" width="132" height="132"><h1>Sign in</h1>
<p class="lead">Confirm with Telegram. MelloID receives your Telegram id, name, username and photo only: never your phone number, login code or session.</p>
<div class="tg-widget"><script async src="https://telegram.org/js/telegram-widget.js?22" data-telegram-login="${bot}" data-size="large" data-radius="12" data-auth-url="${authUrl}"></script></div>
<p class="muted"><a href="/">Back to MelloID</a></p></section></main></body></html>`;
  return new Response(html, {
    headers: {
      "Content-Type": "text/html; charset=utf-8",
      "Cache-Control": "no-store",
      // Only this page may load Telegram's widget script and frame.
      "Content-Security-Policy": "default-src 'self'; script-src https://telegram.org; frame-src https://oauth.telegram.org; style-src 'self' 'unsafe-inline'; img-src 'self' data:; base-uri 'none'; form-action 'none'; frame-ancestors 'none'",
    },
  });
}

const WIDGET_FAILURE_TEXT = {
  expired: "Telegram's sign-in confirmation expired. Please sign in again.",
  missing: "This page is the last step of sign-in with Telegram. Start again from MelloID.",
  bot_token_missing: "Telegram sign-in is not configured on this server yet. Please try again later.",
};

async function finishWidgetLogin(request, env) {
  const url = new URL(request.url);
  const { id, reason } = await verifyTelegramWidgetPayload(url.searchParams, widgetBotToken(env));
  if (!id) {
    // Field names, never values, and never the token: enough for an operator to
    // see which bot answered while keeping the log free of personal data.
    console.warn("MelloID widget sign-in rejected", { reason, fields: [...url.searchParams.keys()].join(",") });
    let message = WIDGET_FAILURE_TEXT[reason]
      || "Telegram's sign-in confirmation could not be verified. Please sign in again.";
    // The most common cause of `signature` is a token for someone else's bot,
    // and naming the bot the token belongs to turns a guess into a two-second
    // check against @BotFather.
    if (reason === "signature") {
      const botId = botIdOf(widgetBotToken(env));
      const owner = botId ? `Telegram bot ${botId}` : "a different bot";
      message += ` The token on this server belongs to ${owner}, not @${env.TELEGRAM_BOT_USERNAME}.`;
    }
    return errorPage(message, 401, reason);
  }
  const sessionId = randomToken();
  const now = Math.floor(Date.now() / 1000);
  await env.DB.prepare(
    "INSERT INTO sessions (session_hash, owner_telegram_id, expires_at, created_at) VALUES (?1, ?2, ?3, ?4)"
  ).bind(await sha256(sessionId), id, now + SESSION_TTL_SECONDS, now).run();
  return redirect(new URL("/dashboard", env.PUBLIC_BASE_URL).toString(), [
    cookieHeader("melloid_session", sessionId, SESSION_TTL_SECONDS),
  ]);
}

async function logout(request, env) {
  if (!requireSameOrigin(request, env)) return json({ error: "Origin check failed." }, 403);
  const session = await getSession(request, env);
  if (session) await env.DB.prepare("DELETE FROM sessions WHERE session_hash = ?1").bind(session.sessionHash).run();
  return json({ ok: true }, 200, { "Set-Cookie": expiredCookie("melloid_session") });
}
async function getOwnProfile(request, env) {
  const session = await requireSession(request, env);
  if (!session) return json({ error: "Sign in with Telegram first." }, 401);
  const row = await env.DB.prepare(
    "SELECT profile_json, slug, visibility, updated_at FROM profiles WHERE owner_telegram_id = ?1"
  ).bind(session.telegramId).first();
  if (!row) return json({ profile: null });
  return json({
    profile: JSON.parse(row.profile_json),
    public_url: new URL(`/@/${row.slug}`, env.PUBLIC_BASE_URL).toString(),
    updated_at: row.updated_at,
  });
}
async function saveOwnProfile(request, env) {
  if (!requireSameOrigin(request, env)) return json({ error: "Origin check failed." }, 403);
  const session = await requireSession(request, env);
  if (!session) return json({ error: "Sign in with Telegram first." }, 401);
  const raw = await request.text();
  if (encoder.encode(raw).length > MAX_PROFILE_BYTES) return json({ error: "Profile export exceeds 256 KiB." }, 413);
  let profile;
  try {
    profile = validateAndSanitizeProfile(JSON.parse(raw));
  } catch (error) {
    return json({ error: error.message || "Invalid profile." }, 400);
  }
  const currentSlugOwner = await env.DB.prepare(
    "SELECT owner_telegram_id FROM profiles WHERE slug = ?1"
  ).bind(profile.slug).first();
  if (currentSlugOwner && currentSlugOwner.owner_telegram_id !== session.telegramId) {
    return json({ error: "That MelloID handle is already in use." }, 409);
  }
  const updatedAt = new Date().toISOString();
  try {
    await env.DB.prepare(
      "INSERT INTO profiles (owner_telegram_id, slug, visibility, profile_json, updated_at) VALUES (?1, ?2, ?3, ?4, ?5) ON CONFLICT(owner_telegram_id) DO UPDATE SET slug = excluded.slug, visibility = excluded.visibility, profile_json = excluded.profile_json, updated_at = excluded.updated_at"
    ).bind(session.telegramId, profile.slug, profile.visibility, JSON.stringify(profile), updatedAt).run();
  } catch {
    return json({ error: "Could not save this handle. It may have been claimed by another user." }, 409);
  }
  return json({
    ok: true,
    slug: profile.slug,
    public_url: new URL(`/@/${profile.slug}`, env.PUBLIC_BASE_URL).toString(),
    updated_at: updatedAt,
  });
}

const STEAM_OPENID_LOGIN_URL = "https://steamcommunity.com/openid/login";
const STEAM_OPENID_NAMESPACE = "http://specs.openid.net/auth/2.0";
const SUPPORTED_CONNECTION_PROVIDERS = new Set(["github", "discord", "steam"]);

const CONNECTION_PROVIDERS = {
  github: {
    clientIdKey: "GITHUB_OAUTH_CLIENT_ID",
    clientSecretKey: "GITHUB_OAUTH_CLIENT_SECRET",
    authorizeUrl: "https://github.com/login/oauth/authorize",
    tokenUrl: "https://github.com/login/oauth/access_token",
    scopes: "read:user",
    label: "GitHub",
  },
  discord: {
    clientIdKey: "DISCORD_OAUTH_CLIENT_ID",
    clientSecretKey: "DISCORD_OAUTH_CLIENT_SECRET",
    authorizeUrl: "https://discord.com/oauth2/authorize",
    tokenUrl: "https://discord.com/api/v10/oauth2/token",
    scopes: "identify",
    label: "Discord",
  },
};

function connectionCookie(provider) {
  return `melloid_connect_state_${provider}`;
}
function requireConnectionProvider(provider, env) {
  const config = CONNECTION_PROVIDERS[provider];
  if (!config) throw new Error("Unsupported connection provider.");
  if (!env[config.clientIdKey] || !env[config.clientSecretKey]) {
    throw new Error(`${config.label} account linking is not configured yet.`);
  }
  return config;
}
function connectionCallbackUrl(provider, env) {
  return new URL(`/auth/connections/${provider}/callback`, env.PUBLIC_BASE_URL).toString();
}

async function storeVerifiedConnection(session, provider, identity, env) {
  const existingOwner = await env.DB.prepare(
    "SELECT owner_telegram_id FROM connections WHERE provider = ?1 AND provider_user_id = ?2"
  ).bind(provider, identity.provider_user_id).first();
  if (existingOwner && existingOwner.owner_telegram_id !== session.telegramId) {
    throw new Error("That external account is already linked to a different MelloID.");
  }
  const now = Math.floor(Date.now() / 1000);
  const verifiedAt = new Date().toISOString();
  await env.DB.prepare(
    "INSERT INTO connections (owner_telegram_id, provider, provider_user_id, username, display_name, profile_url, avatar_url, verified_at, visible, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, ?9, ?9) ON CONFLICT(owner_telegram_id, provider) DO UPDATE SET provider_user_id = excluded.provider_user_id, username = excluded.username, display_name = excluded.display_name, profile_url = excluded.profile_url, avatar_url = excluded.avatar_url, verified_at = excluded.verified_at, visible = 1, updated_at = excluded.updated_at"
  ).bind(session.telegramId, provider, identity.provider_user_id, identity.username, identity.display_name,
    identity.profile_url, identity.avatar_url, verifiedAt, now).run();
  return verifiedAt;
}

async function startConnection(request, env, provider) {
  const session = await requireSession(request, env);
  if (!session) return redirect(new URL("/", env.PUBLIC_BASE_URL).toString());
  const config = requireConnectionProvider(provider, env);
  const state = randomToken();
  const verifier = randomToken(48);
  const challenge = base64url(await crypto.subtle.digest("SHA-256", encoder.encode(verifier)));
  const now = Math.floor(Date.now() / 1000);
  const stateHash = await sha256(state);
  await env.DB.prepare("DELETE FROM oauth_transactions WHERE expires_at <= ?1").bind(now).run();
  await env.DB.prepare(
    "INSERT INTO oauth_transactions (state_hash, provider, owner_telegram_id, session_hash, code_verifier, expires_at, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"
  ).bind(stateHash, provider, session.telegramId, session.sessionHash, verifier, now + 600, now).run();

  const authorize = new URL(config.authorizeUrl);
  authorize.searchParams.set("client_id", env[config.clientIdKey]);
  authorize.searchParams.set("redirect_uri", connectionCallbackUrl(provider, env));
  authorize.searchParams.set("response_type", "code");
  authorize.searchParams.set("scope", config.scopes);
  authorize.searchParams.set("state", state);
  authorize.searchParams.set("code_challenge", challenge);
  authorize.searchParams.set("code_challenge_method", "S256");
  if (provider === "github") authorize.searchParams.set("allow_signup", "false");
  return redirect(authorize.toString(), [cookieHeader(connectionCookie(provider), state, 600)]);
}

async function exchangeConnectionCode(provider, code, verifier, env) {
  const config = requireConnectionProvider(provider, env);
  const callback = connectionCallbackUrl(provider, env);
  const form = new URLSearchParams({
    grant_type: "authorization_code",
    code,
    redirect_uri: callback,
    code_verifier: verifier,
  });
  // GitHub OAuth apps accept client credentials in the form body; Discord
  // uses HTTP Basic client authentication. Do not send both mechanisms.
  if (provider === "github") {
    form.set("client_id", env[config.clientIdKey]);
    form.set("client_secret", env[config.clientSecretKey]);
  }
  const headers = { "Accept": "application/json", "Content-Type": "application/x-www-form-urlencoded" };
  if (provider === "discord") {
    headers.Authorization = "Basic " + btoa(`${env[config.clientIdKey]}:${env[config.clientSecretKey]}`);
  }
  const response = await fetch(config.tokenUrl, { method: "POST", headers, body: form });
  const tokens = await response.json().catch(() => ({}));
  if (!response.ok || typeof tokens.access_token !== "string") {
    console.warn(`${config.label} OAuth code exchange failed`, response.status);
    throw new Error(`${config.label} did not accept the account-link authorization.`);
  }
  return tokens.access_token;
}

async function fetchConnectedIdentity(provider, accessToken) {
  if (provider === "github") {
    const response = await fetch("https://api.github.com/user", {
      headers: {
        "Authorization": `Bearer ${accessToken}`,
        "Accept": "application/vnd.github+json",
        "X-GitHub-Api-Version": "2022-11-28",
        "User-Agent": "MelloID",
      },
    });
    if (!response.ok) throw new Error("GitHub did not return the authorized account profile.");
    return normalizeProviderIdentity("github", await response.json());
  }

  if (provider === "discord") {
    const response = await fetch("https://discord.com/api/v10/users/@me", {
      headers: { "Authorization": `Bearer ${accessToken}` },
    });
    if (!response.ok) throw new Error("Discord did not return the authorized account profile.");
    return normalizeProviderIdentity("discord", await response.json());
  }
  throw new Error("Unsupported account provider.");
}

async function finishConnection(request, env, provider) {
  const config = requireConnectionProvider(provider, env);
  const url = new URL(request.url);
  const state = url.searchParams.get("state");
  const code = url.searchParams.get("code");
  const stateCookie = connectionCookie(provider);
  const cookieState = cookie(request, stateCookie);
  const clearState = expiredCookie(stateCookie);
  if (url.searchParams.has("error")) {
    return redirect(new URL("/dashboard?connection_error=authorization_denied", env.PUBLIC_BASE_URL).toString(), [clearState]);
  }
  if (!state || !code || !cookieState || state !== cookieState) {
    return errorPage("The account-link state expired or did not match. Start the connection again.", 400);
  }
  const session = await requireSession(request, env);
  if (!session) return redirect(new URL("/", env.PUBLIC_BASE_URL).toString(), [clearState]);

  const stateHash = await sha256(state);
  const transaction = await env.DB.prepare(
    "SELECT owner_telegram_id, session_hash, code_verifier, expires_at FROM oauth_transactions WHERE state_hash = ?1 AND provider = ?2"
  ).bind(stateHash, provider).first();
  await env.DB.prepare("DELETE FROM oauth_transactions WHERE state_hash = ?1").bind(stateHash).run();
  const now = Math.floor(Date.now() / 1000);
  if (!transaction || transaction.expires_at <= now
      || transaction.owner_telegram_id !== session.telegramId
      || transaction.session_hash !== session.sessionHash) {
    return errorPage("The account-link request expired or no longer matches this signed-in session.", 400);
  }

  try {
    const accessToken = await exchangeConnectionCode(provider, code, transaction.code_verifier, env);
    // The provider token is used only for this identity lookup. It is never
    // written to D1, exposed to the browser, or retained after this function.
    const identity = await fetchConnectedIdentity(provider, accessToken);
    await storeVerifiedConnection(session, provider, identity, env);
    return redirect(
      new URL(`/dashboard?connection=${provider}&status=connected`, env.PUBLIC_BASE_URL).toString(),
      [clearState]
    );
  } catch (error) {
    console.warn("MelloID connection callback failed", provider, error?.message || "unknown error");
    return redirect(
      new URL(`/dashboard?connection_error=${provider}`, env.PUBLIC_BASE_URL).toString(),
      [clearState]
    );
  }
}


async function startSteamConnection(request, env) {
  const session = await requireSession(request, env);
  if (!session) return redirect(new URL("/", env.PUBLIC_BASE_URL).toString());
  const state = randomToken();
  const stateHash = await sha256(state);
  const now = Math.floor(Date.now() / 1000);
  const callback = new URL("/auth/connections/steam/callback", env.PUBLIC_BASE_URL);
  callback.searchParams.set("state", state);
  const realm = new URL(env.PUBLIC_BASE_URL).origin + "/";
  await env.DB.prepare("DELETE FROM oauth_transactions WHERE expires_at <= ?1").bind(now).run();
  await env.DB.prepare("DELETE FROM steam_openid_nonces WHERE expires_at <= ?1").bind(now).run();
  await env.DB.prepare(
    "INSERT INTO oauth_transactions (state_hash, provider, owner_telegram_id, session_hash, code_verifier, expires_at, created_at) VALUES (?1, 'steam', ?2, ?3, '', ?4, ?5)"
  ).bind(stateHash, session.telegramId, session.sessionHash, now + 600, now).run();

  const authorize = new URL(STEAM_OPENID_LOGIN_URL);
  authorize.searchParams.set("openid.ns", STEAM_OPENID_NAMESPACE);
  authorize.searchParams.set("openid.mode", "checkid_setup");
  authorize.searchParams.set("openid.return_to", callback.toString());
  authorize.searchParams.set("openid.realm", realm);
  authorize.searchParams.set("openid.identity", `${STEAM_OPENID_NAMESPACE}/identifier_select`);
  authorize.searchParams.set("openid.claimed_id", `${STEAM_OPENID_NAMESPACE}/identifier_select`);
  return redirect(authorize.toString(), [cookieHeader(connectionCookie("steam"), state, 600)]);
}

async function verifySteamAssertion(url, expectedReturnTo) {
  const assertion = validateSteamOpenIdResponse(
    url.searchParams,
    expectedReturnTo,
    Math.floor(Date.now() / 1000)
  );
  if (!assertion) return null;

  const form = new URLSearchParams();
  for (const [key, value] of url.searchParams.entries()) {
    if (key.startsWith("openid.") && key !== "openid.mode") form.append(key, value);
  }
  form.set("openid.mode", "check_authentication");
  const response = await fetch(assertion.opEndpoint, {
    method: "POST",
    headers: { "Content-Type": "application/x-www-form-urlencoded" },
    body: form,
  });
  if (!response.ok) return null;
  const text = await response.text();
  if (!text.split(/\r?\n/).some((line) => line.trim() === "is_valid:true")) return null;
  return assertion;
}

async function steamPlayerSummary(steamId, env) {
  if (!env.STEAM_WEB_API_KEY) return null;
  const url = new URL("https://api.steampowered.com/ISteamUser/GetPlayerSummaries/v2/");
  url.searchParams.set("key", env.STEAM_WEB_API_KEY);
  url.searchParams.set("steamids", steamId);
  try {
    const response = await fetch(url);
    if (!response.ok) return null;
    const body = await response.json();
    const player = body.response?.players?.[0];
    return player?.steamid === steamId ? player : null;
  } catch {
    return null;
  }
}

async function finishSteamConnection(request, env) {
  const url = new URL(request.url);
  const state = url.searchParams.get("state");
  const stateCookie = connectionCookie("steam");
  const cookieState = cookie(request, stateCookie);
  const clearState = expiredCookie(stateCookie);
  if (!state || !cookieState || state !== cookieState) {
    return errorPage("The Steam account-link state expired or did not match. Start the connection again.", 400);
  }
  const session = await requireSession(request, env);
  if (!session) return redirect(new URL("/", env.PUBLIC_BASE_URL).toString(), [clearState]);

  const stateHash = await sha256(state);
  const transaction = await env.DB.prepare(
    "SELECT owner_telegram_id, session_hash, expires_at FROM oauth_transactions WHERE state_hash = ?1 AND provider = 'steam'"
  ).bind(stateHash).first();
  await env.DB.prepare("DELETE FROM oauth_transactions WHERE state_hash = ?1").bind(stateHash).run();
  const now = Math.floor(Date.now() / 1000);
  if (!transaction || transaction.expires_at <= now
      || transaction.owner_telegram_id !== session.telegramId
      || transaction.session_hash !== session.sessionHash) {
    return errorPage("The Steam account-link request expired or no longer matches this signed-in session.", 400);
  }

  try {
    const callback = new URL("/auth/connections/steam/callback", env.PUBLIC_BASE_URL);
    callback.searchParams.set("state", state);
    const assertion = await verifySteamAssertion(url, callback.toString());
    if (!assertion) throw new Error("Steam OpenID assertion could not be verified.");

    const nonceHash = await sha256(assertion.responseNonce);
    await env.DB.prepare(
      "INSERT INTO steam_openid_nonces (nonce_hash, expires_at, created_at) VALUES (?1, ?2, ?3)"
    ).bind(nonceHash, now + 600, now).run();

    const summary = await steamPlayerSummary(assertion.steamId, env);
    const identity = normalizeSteamIdentity(assertion.steamId, summary);
    await storeVerifiedConnection(session, "steam", identity, env);
    return redirect(
      new URL("/dashboard?connection=steam&status=connected", env.PUBLIC_BASE_URL).toString(),
      [clearState]
    );
  } catch (error) {
    console.warn("MelloID Steam OpenID callback failed", error?.message || "unknown error");
    return redirect(
      new URL("/dashboard?connection_error=steam", env.PUBLIC_BASE_URL).toString(),
      [clearState]
    );
  }
}

async function getOwnConnections(session, env) {
  const result = await env.DB.prepare(
    "SELECT provider, provider_user_id, username, display_name, profile_url, avatar_url, verified_at, visible FROM connections WHERE owner_telegram_id = ?1 ORDER BY provider"
  ).bind(session.telegramId).all();
  return (result.results || []);
}
async function updateConnectionVisibility(request, env, session, provider) {
  if (!requireSameOrigin(request, env)) return json({ error: "Origin check failed." }, 403);
  if (!SUPPORTED_CONNECTION_PROVIDERS.has(provider)) return json({ error: "Unsupported connection provider." }, 400);
  let body;
  try { body = await request.json(); } catch { return json({ error: "Invalid JSON body." }, 400); }
  if (!body || typeof body.visible !== "boolean") return json({ error: "visible must be a boolean." }, 400);
  await env.DB.prepare(
    "UPDATE connections SET visible = ?1, updated_at = ?2 WHERE owner_telegram_id = ?3 AND provider = ?4"
  ).bind(body.visible ? 1 : 0, Math.floor(Date.now() / 1000), session.telegramId, provider).run();
  return json({ ok: true, visible: body.visible });
}
async function unlinkConnection(request, env, session, provider) {
  if (!requireSameOrigin(request, env)) return json({ error: "Origin check failed." }, 403);
  if (!CONNECTION_PROVIDERS[provider]) return json({ error: "Unsupported connection provider." }, 400);
  await env.DB.prepare(
    "DELETE FROM connections WHERE owner_telegram_id = ?1 AND provider = ?2"
  ).bind(session.telegramId, provider).run();
  return json({ ok: true });
}

async function getPublicProfile(slug, env) {
  if (!/^[a-z0-9-]{1,32}$/.test(slug)) return json({ error: "Profile not found." }, 404);
  const row = await env.DB.prepare(
    "SELECT owner_telegram_id, profile_json, visibility, updated_at FROM profiles WHERE slug = ?1"
  ).bind(slug).first();
  if (!row || row.visibility === "private") return json({ error: "Profile not found." }, 404);
  const linked = await env.DB.prepare(
    "SELECT provider, provider_user_id, username, display_name, profile_url, avatar_url, verified_at, visible FROM connections WHERE owner_telegram_id = ?1 AND visible = 1"
  ).bind(row.owner_telegram_id).all();
  const profile = publicProfileProjection(JSON.parse(row.profile_json), linked.results || []);
  if (!profile) return json({ error: "Profile not found." }, 404);
  return json({ profile, updated_at: row.updated_at }, 200, { "Cache-Control": "public, max-age=60" });
}
/**
 * Global badges for MelloGram clients: given up to 50 Telegram user ids,
 * returns the visible badges and handle of each one that has a PUBLIC
 * MelloID profile. Unlisted and private profiles are never matched, so this
 * reveals nothing a public profile page does not already show.
 */
export function parseBadgeIds(raw) {
  const ids = [...new Set(String(raw || "").split(",").map((v) => v.trim()).filter((v) => /^[1-9][0-9]{0,15}$/.test(v)))];
  return ids.slice(0, 50);
}
async function getBadges(url, env) {
  const ids = parseBadgeIds(url.searchParams.get("ids"));
  if (!ids.length) return json({ users: {} }, 200, { "Cache-Control": "public, max-age=300" });
  const placeholders = ids.map((_, i) => `?${i + 1}`).join(",");
  const rows = await env.DB.prepare(
    `SELECT owner_telegram_id, slug, profile_json FROM profiles WHERE visibility = 'public' AND owner_telegram_id IN (${placeholders})`
  ).bind(...ids).all();
  const users = {};
  for (const row of rows.results || []) {
    let profile;
    try { profile = JSON.parse(row.profile_json); } catch { continue; }
    const badges = (profile.badges || []).filter((b) => b && b.visible).slice(0, 6)
      .map((b) => ({ title: String(b.title || "").slice(0, 48), description: String(b.description || "").slice(0, 160) }));
    users[row.owner_telegram_id] = { slug: row.slug, badges };
  }
  return json({ users }, 200, { "Cache-Control": "public, max-age=300" });
}
async function staticAsset(path, request, env) {
  return env.ASSETS.fetch(new Request(new URL(path, request.url), request));
}

export default {
  async fetch(request, env) {
    try {
      const url = new URL(request.url);
      const path = url.pathname;
      // HTTPS only (local development excepted): redirect reads, refuse writes.
      if (url.protocol === "http:" && !["localhost", "127.0.0.1", "[::1]"].includes(url.hostname)) {
        if (request.method !== "GET" && request.method !== "HEAD") return setSecurityHeaders(json({ error: "HTTPS is required." }, 403));
        url.protocol = "https:";
        return setSecurityHeaders(new Response(null, { status: 308, headers: { Location: url.toString() } }));
      }
      // Telegram sign-in: OpenID Connect when its client is configured,
      // otherwise the Login Widget; a clear 503 when neither is.
      if (path === "/login" && request.method === "GET") {
        if (!widgetConfigured(env)) return setSecurityHeaders(json({ error: "Telegram sign-in is not configured on this MelloID server yet." }, 503));
        return setSecurityHeaders(widgetLoginPage(env));
      }
      if (path === "/auth/telegram/widget" && request.method === "GET") {
        if (!widgetConfigured(env)) return setSecurityHeaders(json({ error: "Telegram sign-in is not configured on this MelloID server yet." }, 503));
        return setSecurityHeaders(await finishWidgetLogin(request, env));
      }
      if (path.startsWith("/auth/telegram/") && !oidcConfigured(env)) {
        if (path === "/auth/telegram/start" && widgetConfigured(env)) return setSecurityHeaders(redirect(new URL("/login", env.PUBLIC_BASE_URL).toString(), []));
        return setSecurityHeaders(json({ error: "Telegram sign-in is not configured on this MelloID server yet." }, 503));
      }
      if (path === "/auth/telegram/start" && request.method === "GET") return setSecurityHeaders(await startTelegramLogin(request, env));
      if (path === "/auth/telegram/callback" && request.method === "GET") return setSecurityHeaders(await finishTelegramLogin(request, env));
      const connectionStart = path.match(/^\/auth\/connections\/(github|discord|steam)\/start$/);
      if (connectionStart && request.method === "GET") {
        const provider = connectionStart[1];
        return setSecurityHeaders(await (provider === "steam"
          ? startSteamConnection(request, env) : startConnection(request, env, provider)));
      }
      const connectionCallback = path.match(/^\/auth\/connections\/(github|discord|steam)\/callback$/);
      if (connectionCallback && request.method === "GET") {
        const provider = connectionCallback[1];
        return setSecurityHeaders(await (provider === "steam"
          ? finishSteamConnection(request, env) : finishConnection(request, env, provider)));
      }
      if (path === "/auth/logout" && request.method === "POST") return setSecurityHeaders(await logout(request, env));
      if (path === "/api/v1/me/connections" && request.method === "GET") {
        const session = await requireSession(request, env);
        if (!session) return json({ error: "Sign in with Telegram first." }, 401);
        return setSecurityHeaders(json({ connections: await getOwnConnections(session, env) }));
      }
      const connectionVisibility = path.match(/^\/api\/v1\/me\/connections\/(github|discord|steam)$/);
      if (connectionVisibility && request.method === "PUT") {
        const session = await requireSession(request, env);
        if (!session) return json({ error: "Sign in with Telegram first." }, 401);
        return setSecurityHeaders(await updateConnectionVisibility(request, env, session, connectionVisibility[1]));
      }
      if (connectionVisibility && request.method === "DELETE") {
        const session = await requireSession(request, env);
        if (!session) return json({ error: "Sign in with Telegram first." }, 401);
        return setSecurityHeaders(await unlinkConnection(request, env, session, connectionVisibility[1]));
      }
      if (path === "/api/v1/me/profile" && request.method === "GET") return setSecurityHeaders(await getOwnProfile(request, env));
      if (path === "/api/v1/me/profile" && request.method === "PUT") return setSecurityHeaders(await saveOwnProfile(request, env));
      if (path === "/api/v1/me/profile" && request.method === "DELETE") {
        if (!requireSameOrigin(request, env)) return json({ error: "Origin check failed." }, 403);
        const session = await requireSession(request, env);
        if (!session) return json({ error: "Sign in with Telegram first." }, 401);
        await env.DB.prepare("DELETE FROM profiles WHERE owner_telegram_id = ?1").bind(session.telegramId).run();
        await env.DB.prepare("DELETE FROM connections WHERE owner_telegram_id = ?1").bind(session.telegramId).run();
        await env.DB.prepare("DELETE FROM oauth_transactions WHERE owner_telegram_id = ?1").bind(session.telegramId).run();
        return setSecurityHeaders(json({ ok: true }));
      }
      if (path === "/api/v1/badges" && request.method === "GET") return setSecurityHeaders(await getBadges(url, env));
      if (path.startsWith("/api/v1/profiles/") && request.method === "GET") {
        const slug = decodeURIComponent(path.slice("/api/v1/profiles/".length));
        return setSecurityHeaders(await getPublicProfile(slug, env));
      }
      if (path.startsWith("/@/") && request.method === "GET") return setSecurityHeaders(await staticAsset("/profile.html", request, env));
      if (path === "/dashboard" || path === "/dashboard/") return setSecurityHeaders(await staticAsset("/dashboard.html", request, env));
      // The asset layer serves files by their exact path (html_handling = none),
      // so the home page is named explicitly instead of relying on extension
      // mapping that would rewrite it.
      if (path === "/" || path === "/index.html") return setSecurityHeaders(await staticAsset("/index.html", request, env));
      return setSecurityHeaders(await env.ASSETS.fetch(request));
    } catch (error) {
      console.error("MelloID Worker error", error);
      return setSecurityHeaders(json({ error: "The MelloID service encountered an internal error." }, 500));
    }
  },
};
