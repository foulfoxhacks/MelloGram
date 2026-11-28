// MelloGram web client: serves the static shell and a small JSON API.
//
// This is a skeleton, and it says so: no Telegram session, no message storage
// and no overlay until the browser build can host them honestly. The Worker is
// here so the shell, the deployment target and the security headers are real
// from the start.

import { errorJson, json, status } from "./capabilities.js";
import { relayFetch } from "./relay.js";

// Two hosts, one Worker:
//   wc.mellogram.foulfoxhacks.dev      the shell (and /api/* for same-origin use)
//   wc-api.mellogram.foulfoxhacks.dev  the API only; nothing else is served there
// Both are HTTPS-only. Plain HTTP is redirected (or refused for writes), and
// every response carries HSTS. The API answers cross-origin requests only
// from the web client's own origin.

const DEFAULT_API = "https://wc-api.mellogram.foulfoxhacks.dev";

const csp = (apiOrigin) => [
  "default-src 'self'",
  "img-src 'self' data:",
  "style-src 'self'",
  "script-src 'self' 'wasm-unsafe-eval'",
  // The relay is a WebSocket; the page may only open it on our own API host.
  `connect-src 'self'${apiOrigin ? ` ${apiOrigin} ${apiOrigin.replace(/^http/, "ws")}` : ""}`,
  "base-uri 'none'",
  "form-action 'none'",
  "frame-ancestors 'none'",
  "upgrade-insecure-requests",
].join("; ");

const originOf = (value) => {
  try { return value ? new URL(value).origin : null; } catch { return null; }
};
const isLocal = (url) => ["localhost", "127.0.0.1", "[::1]"].includes(url.hostname);

export function withSecurityHeaders(response, env = {}) {
  const headers = new Headers(response.headers);
  headers.set("X-Content-Type-Options", "nosniff");
  headers.set("Referrer-Policy", "strict-origin-when-cross-origin");
  headers.set("X-Frame-Options", "DENY");
  headers.set("Permissions-Policy", "camera=(), microphone=(), geolocation=()");
  headers.set("Cross-Origin-Opener-Policy", "same-origin");
  headers.set("Content-Security-Policy", csp(originOf(env.API_BASE_URL || DEFAULT_API)));
  headers.set("Strict-Transport-Security", "max-age=63072000");
  headers.set("Cross-Origin-Resource-Policy", "same-site");
  return new Response(response.body, {
    status: response.status,
    statusText: response.statusText,
    headers,
  });
}

function cors(request, response, env) {
  const allowed = originOf(env.PUBLIC_BASE_URL);
  const origin = request.headers.get("Origin");
  response.headers.append("Vary", "Origin");
  if (origin && allowed && origin === allowed) {
    response.headers.set("Access-Control-Allow-Origin", allowed);
    response.headers.set("Access-Control-Allow-Methods", "GET, HEAD, OPTIONS");
    response.headers.set("Access-Control-Allow-Headers", "Accept, Content-Type");
    response.headers.set("Access-Control-Max-Age", "600");
  }
  return response;
}

async function api(request, url, env) {
  if (request.method === "OPTIONS") {
    const allowed = originOf(env.PUBLIC_BASE_URL);
    const ok = allowed && request.headers.get("Origin") === allowed;
    return new Response(null, { status: ok ? 204 : 403 });
  }
  if (request.method !== "GET" && request.method !== "HEAD") {
    const response = errorJson("method not allowed", 405);
    response.headers.set("Allow", "GET, HEAD, OPTIONS");
    return response;
  }
  if (url.pathname === "/api/health") {
    return json({ ok: true, service: "mellogram-web-client" });
  }
  if (url.pathname === "/api/status") {
    return json(status(env));
  }
  return errorJson("no such endpoint", 404);
}

const isApiHost = (url, env) => {
  const api = originOf(env.API_BASE_URL || DEFAULT_API);
  return api !== null && url.origin === api;
};

export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    const done = (response) => withSecurityHeaders(response, env);

    // HTTPS only (local development excepted).
    if (url.protocol === "http:" && !isLocal(url)) {
      if (request.method !== "GET" && request.method !== "HEAD") return done(errorJson("HTTPS is required", 403));
      url.protocol = "https:";
      return done(new Response(null, { status: 308, headers: { Location: url.toString() } }));
    }

    // MTProto transport relay: /relay/dc/<1-5>. A 101 upgrade carries the
    // socket itself, so it is returned unwrapped: copying the response into a
    // new one would drop the WebSocket.
    const relay = url.pathname.match(/^\/relay\/dc\/([^/]+)$/);
    if (relay) return relayFetch(request, env, relay[1]);

    if (isApiHost(url, env)) {
      if (url.pathname === "/" || url.pathname === "") {
        return done(cors(request, json({ service: "mellogram-web-client-api", endpoints: ["/api/health", "/api/status"] }), env));
      }
      if (!url.pathname.startsWith("/api/")) return done(errorJson("no such endpoint", 404));
      return done(cors(request, await api(request, url, env), env));
    }

    if (url.pathname.startsWith("/api/")) {
      return done(cors(request, await api(request, url, env), env));
    }
    if (request.method !== "GET" && request.method !== "HEAD") {
      const response = errorJson("method not allowed", 405);
      response.headers.set("Allow", "GET, HEAD");
      return done(response);
    }
    if (!env.ASSETS || typeof env.ASSETS.fetch !== "function") {
      return done(
        new Response("Static assets are not configured for this deployment.\n", {
          status: 503,
          headers: { "Content-Type": "text/plain; charset=utf-8" },
        }),
      );
    }
    return done(await env.ASSETS.fetch(request));
  },
};
