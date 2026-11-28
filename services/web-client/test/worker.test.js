import test from "node:test";
import assert from "node:assert/strict";

import worker, { withSecurityHeaders } from "../src/worker.js";
import { capabilities } from "../src/capabilities.js";

const shell = () =>
  new Response("<!doctype html><title>MelloGram Web</title>", {
    headers: { "Content-Type": "text/html; charset=utf-8" },
  });

function env(overrides = {}) {
  return {
    PUBLIC_BASE_URL: "https://wc.example.com",
    ASSETS: { fetch: async () => shell() },
    ...overrides,
  };
}

const call = (path, init) =>
  worker.fetch(new Request(`https://wc.example.com${path}`, init), env());

test("health and status answer JSON with security headers", async () => {
  const health = await call("/api/health");
  assert.equal(health.status, 200);
  assert.equal(health.headers.get("content-type"), "application/json; charset=utf-8");
  assert.equal(health.headers.get("cache-control"), "no-store");
  assert.match(health.headers.get("content-security-policy"), /default-src 'self'/);
  assert.equal((await health.json()).ok, true);

  const status = await call("/api/status");
  const body = await status.json();
  assert.equal(body.service, "mellogram-web-client");
  assert.equal(body.stage, "beta");
  assert.equal(body.telegramBeta, true);
  assert.equal(body.origin, "https://wc.example.com");
  assert.equal(body.capabilities.length, capabilities().length);
});

test("unfinished capabilities are reported as not ready", async () => {
  const body = await (await call("/api/status")).json();
  for (const id of ["login", "messaging", "overlay"]) {
    const entry = body.capabilities.find((item) => item.id === id);
    assert.ok(entry, id);
    assert.equal(entry.ready, false, id);
    assert.ok(entry.detail.length > 0, id);
  }
  assert.equal(body.capabilities.find((item) => item.id === "shell").ready, true);
});

test("mutating API requests are refused with Allow", async () => {
  const response = await call("/api/status", { method: "POST" });
  assert.equal(response.status, 405);
  assert.equal(response.headers.get("allow"), "GET, HEAD, OPTIONS");
});

test("unknown API endpoints are JSON 404s, never the shell", async () => {
  const response = await call("/api/nope");
  assert.equal(response.status, 404);
  assert.match(response.headers.get("content-type"), /application\/json/);
});

test("static requests go to the assets binding", async () => {
  let seen = null;
  const workerFetch = await worker.fetch(
    new Request("https://wc.example.com/"),
    env({
      ASSETS: {
        fetch: async (request) => {
          seen = request.url;
          return shell();
        },
      },
    }),
  );
  assert.equal(seen, "https://wc.example.com/");
  assert.equal(workerFetch.status, 200);
  assert.equal(workerFetch.headers.get("x-frame-options"), "DENY");
});

test("a missing assets binding is a clear 503, not a crash", async () => {
  const response = await worker.fetch(
    new Request("https://wc.example.com/"),
    { PUBLIC_BASE_URL: "https://wc.example.com" },
  );
  assert.equal(response.status, 503);
});

test("withSecurityHeaders keeps the body and adds the policy", async () => {
  const response = withSecurityHeaders(new Response("ok", { status: 201 }));
  assert.equal(response.status, 201);
  assert.equal(await response.text(), "ok");
  assert.equal(response.headers.get("permissions-policy").includes("camera=()"), true);
});

const apiEnv = () => env({ API_BASE_URL: "https://wc-api.example.com" });
const callApi = (path, init) => worker.fetch(new Request(`https://wc-api.example.com${path}`, init), apiEnv());

test("the API host serves only the API", async () => {
  const status = await callApi("/api/status");
  assert.equal(status.status, 200);
  assert.equal((await status.json()).api, "https://wc-api.example.com");
  const page = await callApi("/index.html");
  assert.equal(page.status, 404);
  assert.equal((await callApi("/")).status, 200);
});

test("CORS is granted to the web client origin only", async () => {
  const ok = await callApi("/api/status", { headers: { Origin: "https://wc.example.com" } });
  assert.equal(ok.headers.get("access-control-allow-origin"), "https://wc.example.com");
  assert.match(ok.headers.get("vary"), /Origin/);
  const other = await callApi("/api/status", { headers: { Origin: "https://evil.example" } });
  assert.equal(other.headers.get("access-control-allow-origin"), null);
  const preflight = await callApi("/api/status", { method: "OPTIONS", headers: { Origin: "https://evil.example" } });
  assert.equal(preflight.status, 403);
  const good = await callApi("/api/status", { method: "OPTIONS", headers: { Origin: "https://wc.example.com" } });
  assert.equal(good.status, 204);
});

test("plain HTTP is upgraded and HSTS is always sent", async () => {
  const redirect = await worker.fetch(new Request("http://wc.example.com/chat?x=1"), env());
  assert.equal(redirect.status, 308);
  assert.equal(redirect.headers.get("location"), "https://wc.example.com/chat?x=1");
  const post = await worker.fetch(new Request("http://wc.example.com/api/status", { method: "POST" }), env());
  assert.equal(post.status, 403);
  const page = await call("/");
  assert.match(page.headers.get("strict-transport-security"), /max-age=\d+/);
  assert.match(page.headers.get("content-security-policy"), /upgrade-insecure-requests/);
});

test("the Telegram beta is on by default, can be disabled, and validates overrides", async () => {
  const configured = {
    TELEGRAM_API_ID: "1234",
    TELEGRAM_API_HASH: "a".repeat(32),
  };
  const fetch = (path, overrides, init) =>
    worker.fetch(new Request(`https://wc.example.com${path}`, init), env(overrides));
  // Nothing configured: the deployment uses MelloGram's own public application.
  const builtin = await (await fetch("/api/telegram-config", {})).json();
  assert.equal(builtin.apiId, 30886269);
  assert.match(builtin.apiHash, /^[a-f\d]{32}$/);
  for (const off of ["false", "FALSE", "0", "off"]) {
    const disabled = { TELEGRAM_WEB_BETA: off };
    assert.equal((await fetch("/api/telegram-config", disabled)).status, 404, off);
    assert.equal((await (await fetch("/api/status", disabled)).json()).telegramBeta, false, off);
  }
  for (const bad of [
    { ...configured, TELEGRAM_API_ID: "2147483648" },
    { ...configured, TELEGRAM_API_HASH: "invalid" },
    { TELEGRAM_API_ID: "1234" },
  ]) {
    assert.equal((await fetch("/api/telegram-config", bad)).status, 404);
    assert.equal((await (await fetch("/api/status", bad)).json()).telegramBeta, false);
  }
  const response = await fetch("/api/telegram-config", configured);
  assert.equal(response.headers.get("cache-control"), "no-store");
  assert.deepEqual(await response.json(), { apiId: 1234, apiHash: "a".repeat(32) });
  const status = await (await fetch("/api/status", configured)).json();
  assert.equal(status.stage, "beta");
  assert.equal(status.telegramBeta, true);
  assert.equal(status.capabilities.find(({ id }) => id === "login").beta, true);
  assert.equal(status.capabilities.find(({ id }) => id === "session").ready, true);
  assert.equal(status.capabilities.find(({ id }) => id === "overlay").ready, false);
  assert.equal((await fetch("/api/telegram-config", configured, { method: "POST" })).status, 405);
  assert.equal((await worker.fetch(new Request("https://wc-api.mellogram.foulfoxhacks.dev/api/telegram-config"), env(configured))).status, 404);
});
