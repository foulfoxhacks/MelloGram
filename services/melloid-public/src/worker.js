import { createRemoteJWKSet, jwtVerify } from "jose";
import { publicProfileProjection, validateAndSanitizeProfile } from "./profile.js";
import { normalizeProviderIdentity } from "./connections.js";

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
function errorPage(message, status = 400) {
  return new Response(`<!doctype html><title>MelloID sign-in error</title><main><h1>Sign-in failed</h1><p>${String(message).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c])}</p><a href="/">Return home</a></main>`, {
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
  if (headers.get("Content-Type")?.includes("text/html")) {
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
    const existingOwner = await env.DB.prepare(
      "SELECT owner_telegram_id FROM connections WHERE provider = ?1 AND provider_user_id = ?2"
    ).bind(provider, identity.provider_user_id).first();
    if (existingOwner && existingOwner.owner_telegram_id !== session.telegramId) {
      return errorPage("That external account is already linked to a different MelloID.", 409);
    }
    const verifiedAt = new Date().toISOString();
    await env.DB.prepare(
      "INSERT INTO connections (owner_telegram_id, provider, provider_user_id, username, display_name, profile_url, avatar_url, verified_at, visible, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, ?9, ?9) ON CONFLICT(owner_telegram_id, provider) DO UPDATE SET provider_user_id = excluded.provider_user_id, username = excluded.username, display_name = excluded.display_name, profile_url = excluded.profile_url, avatar_url = excluded.avatar_url, verified_at = excluded.verified_at, visible = 1, updated_at = excluded.updated_at"
    ).bind(session.telegramId, provider, identity.provider_user_id, identity.username, identity.display_name,
      identity.profile_url, identity.avatar_url, verifiedAt, now).run();
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

async function getOwnConnections(session, env) {
  const result = await env.DB.prepare(
    "SELECT provider, provider_user_id, username, display_name, profile_url, avatar_url, verified_at, visible FROM connections WHERE owner_telegram_id = ?1 ORDER BY provider"
  ).bind(session.telegramId).all();
  return (result.results || []);
}
async function updateConnectionVisibility(request, env, session, provider) {
  if (!requireSameOrigin(request, env)) return json({ error: "Origin check failed." }, 403);
  if (!CONNECTION_PROVIDERS[provider]) return json({ error: "Unsupported connection provider." }, 400);
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
async function staticAsset(path, request, env) {
  return env.ASSETS.fetch(new Request(new URL(path, request.url), request));
}

export default {
  async fetch(request, env) {
    try {
      const url = new URL(request.url);
      const path = url.pathname;
      if (path === "/auth/telegram/start" && request.method === "GET") return setSecurityHeaders(await startTelegramLogin(request, env));
      if (path === "/auth/telegram/callback" && request.method === "GET") return setSecurityHeaders(await finishTelegramLogin(request, env));
      const connectionStart = path.match(/^\/auth\/connections\/(github|discord)\/start$/);
      if (connectionStart && request.method === "GET") return setSecurityHeaders(await startConnection(request, env, connectionStart[1]));
      const connectionCallback = path.match(/^\/auth\/connections\/(github|discord)\/callback$/);
      if (connectionCallback && request.method === "GET") return setSecurityHeaders(await finishConnection(request, env, connectionCallback[1]));
      if (path === "/auth/logout" && request.method === "POST") return setSecurityHeaders(await logout(request, env));
      if (path === "/api/v1/me/connections" && request.method === "GET") {
        const session = await requireSession(request, env);
        if (!session) return json({ error: "Sign in with Telegram first." }, 401);
        return setSecurityHeaders(json({ connections: await getOwnConnections(session, env) }));
      }
      const connectionVisibility = path.match(/^\/api\/v1\/me\/connections\/(github|discord)$/);
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
      if (path.startsWith("/api/v1/profiles/") && request.method === "GET") {
        const slug = decodeURIComponent(path.slice("/api/v1/profiles/".length));
        return setSecurityHeaders(await getPublicProfile(slug, env));
      }
      if (path.startsWith("/@/") && request.method === "GET") return setSecurityHeaders(await staticAsset("/profile.html", request, env));
      if (path === "/dashboard" || path === "/dashboard/") return setSecurityHeaders(await staticAsset("/dashboard.html", request, env));
      return setSecurityHeaders(await env.ASSETS.fetch(request));
    } catch (error) {
      console.error("MelloID Worker error", error);
      return setSecurityHeaders(json({ error: "The MelloID service encountered an internal error." }, 500));
    }
  },
};
