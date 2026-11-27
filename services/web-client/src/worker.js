// MelloGram web client: serves the static shell and a small JSON API.
//
// This is a skeleton, and it says so: no Telegram session, no message storage
// and no overlay until the browser build can host them honestly. The Worker is
// here so the shell, the deployment target and the security headers are real
// from the start.

import { errorJson, json, status } from "./capabilities.js";

const CSP = [
  "default-src 'self'",
  "img-src 'self' data:",
  "style-src 'self'",
  "script-src 'self' 'wasm-unsafe-eval'",
  "connect-src 'self'",
  "base-uri 'none'",
  "form-action 'none'",
  "frame-ancestors 'none'",
].join("; ");

export function withSecurityHeaders(response) {
  const headers = new Headers(response.headers);
  headers.set("X-Content-Type-Options", "nosniff");
  headers.set("Referrer-Policy", "strict-origin-when-cross-origin");
  headers.set("X-Frame-Options", "DENY");
  headers.set("Permissions-Policy", "camera=(), microphone=(), geolocation=()");
  headers.set("Cross-Origin-Opener-Policy", "same-origin");
  headers.set("Content-Security-Policy", CSP);
  return new Response(response.body, {
    status: response.status,
    statusText: response.statusText,
    headers,
  });
}

async function api(request, url, env) {
  if (request.method !== "GET" && request.method !== "HEAD") {
    const response = errorJson("method not allowed", 405);
    response.headers.set("Allow", "GET, HEAD");
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

export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    if (url.pathname.startsWith("/api/")) {
      return withSecurityHeaders(await api(request, url, env));
    }
    if (request.method !== "GET" && request.method !== "HEAD") {
      const response = errorJson("method not allowed", 405);
      response.headers.set("Allow", "GET, HEAD");
      return withSecurityHeaders(response);
    }
    if (!env.ASSETS || typeof env.ASSETS.fetch !== "function") {
      return withSecurityHeaders(
        new Response("Static assets are not configured for this deployment.\n", {
          status: 503,
          headers: { "Content-Type": "text/plain; charset=utf-8" },
        }),
      );
    }
    return withSecurityHeaders(await env.ASSETS.fetch(request));
  },
};
