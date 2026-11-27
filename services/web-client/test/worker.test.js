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
  assert.equal(body.stage, "skeleton");
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
  assert.equal(response.headers.get("allow"), "GET, HEAD");
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
