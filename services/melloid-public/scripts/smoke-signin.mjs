import assert from "node:assert/strict";

const origin = new URL(process.argv[2]);
const fetchRoute = (url) => fetch(url, { redirect: "manual", signal: AbortSignal.timeout(10000) });
const start = await fetchRoute(new URL("/auth/telegram/start", origin));
assert.equal(start.status, 303, "MelloID sign-in must be configured");
const location = new URL(start.headers.get("location"));
if (location.origin === origin.origin && location.pathname === "/login") {
  const login = await fetchRoute(location);
  assert.equal(login.status, 200);
  const html = await login.text();
  assert.match(html, /data-telegram-login="[A-Za-z0-9_]+"/);
  const callback = new URL(html.match(/data-auth-url="([^"]+)"/)[1]);
  assert.equal(callback.origin, origin.origin);
  assert.match(callback.pathname, /^\/auth\/telegram\/widget\/[A-Za-z0-9_-]{43}$/);
  const cookie = login.headers.get("set-cookie") ?? "";
  assert.equal(cookie.split(";")[0], `__Host-melloid-widget-state=${callback.pathname.split("/").at(-1)}`);
  const withoutState = await fetchRoute(callback);
  assert.equal(withoutState.status, 400);
  assert.equal(withoutState.headers.get("set-cookie"), null);
} else {
  assert.equal(location.origin, "https://oauth.telegram.org");
  assert.equal(location.pathname, "/auth");
  assert.equal(location.searchParams.get("redirect_uri"), new URL("/auth/telegram/callback", origin).href);
  assert.equal(location.searchParams.get("code_challenge_method"), "S256");
  assert.ok(location.searchParams.get("state"));
}
console.log("MelloID sign-in entry point passed");
