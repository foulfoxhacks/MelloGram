// Cloudflare Worker for the MelloGram website.
//
// The site itself is generated into `dist/` by `site/build.mjs`; this Worker is
// the edge in front of it: security headers, cache policy, a real HTML 404 for
// page requests, and method handling. Nothing is rendered here, so the static
// build stays the single source of truth.

const CSP = [
  "default-src 'self'",
  "img-src 'self' data:",
  "style-src 'self'",
  "script-src 'self'",
  "connect-src 'self'",
  "base-uri 'none'",
  "form-action 'none'",
  "frame-ancestors 'none'",
].join("; ");

export function cacheControl(pathname) {
  if (pathname.startsWith("/assets/")) return "public, max-age=31536000, immutable";
  if (pathname === "/" || pathname.endsWith("/") || pathname.endsWith(".html")) return "no-cache";
  if (pathname === "/sitemap.xml" || pathname === "/robots.txt" || pathname === "/site.webmanifest") {
    return "public, max-age=3600";
  }
  return "public, max-age=86400";
}

export function withSecurityHeaders(response, pathname) {
  const headers = new Headers(response.headers);
  headers.set("Content-Security-Policy", CSP);
  headers.set("X-Content-Type-Options", "nosniff");
  headers.set("Referrer-Policy", "strict-origin-when-cross-origin");
  headers.set("X-Frame-Options", "DENY");
  headers.set("Cross-Origin-Opener-Policy", "same-origin");
  headers.set("Permissions-Policy", "camera=(), microphone=(), geolocation=()");
  headers.set("Cache-Control", cacheControl(pathname));
  return new Response(response.body, {
    status: response.status,
    statusText: response.statusText,
    headers,
  });
}

const wantsHtml = (request, pathname) =>
  (request.headers.get("Accept") || "").includes("text/html") ||
  pathname === "/" ||
  pathname.endsWith("/");

export default {
  async fetch(request, env) {
    const url = new URL(request.url);

    if (request.method !== "GET" && request.method !== "HEAD") {
      return withSecurityHeaders(
        new Response("Method not allowed", {
          status: 405,
          headers: { "Content-Type": "text/plain; charset=utf-8", Allow: "GET, HEAD" },
        }),
        url.pathname,
      );
    }

    if (!env.ASSETS || typeof env.ASSETS.fetch !== "function") {
      return withSecurityHeaders(
        new Response("Static assets are not configured for this deployment.\n", {
          status: 503,
          headers: { "Content-Type": "text/plain; charset=utf-8" },
        }),
        url.pathname,
      );
    }

    let response = await env.ASSETS.fetch(request);

    // A page request for something that does not exist gets the real 404 page,
    // not the platform's blank one.
    if (response.status === 404 && wantsHtml(request, url.pathname)) {
      const page = await env.ASSETS.fetch(new Request(new URL("/404.html", url), { headers: request.headers }));
      if (page.ok) {
        response = new Response(page.body, {
          status: 404,
          headers: { "Content-Type": "text/html; charset=utf-8" },
        });
      }
    }

    return withSecurityHeaders(response, url.pathname);
  },
};
