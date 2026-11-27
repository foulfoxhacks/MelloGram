import test from "node:test";
import assert from "node:assert/strict";

import worker, { cacheControl } from "../worker.mjs";

const page = (body, type = "text/html; charset=utf-8") => new Response(body, { headers: { "Content-Type": type } });

function env(routes) {
  return {
    ASSETS: {
      async fetch(request) {
        const path = new URL(request.url).pathname;
        const hit = routes[path];
        if (!hit) return new Response("missing", { status: 404 });
        if (hit instanceof Response) return hit;
        return typeof hit === "function" ? hit() : page(hit);
      },
    },
  };
}

const call = (path, init, routes = { "/": page("<!doctype html><title>home</title>") }) =>
  worker.fetch(new Request(`https://mellogram.example${path}`, init), env(routes));

test("pages are served with security headers and no-cache", async () => {
  const response = await call("/", { headers: { Accept: "text/html" } });
  assert.equal(response.status, 200);
  assert.match(response.headers.get("content-security-policy"), /default-src 'self'/);
  assert.equal(response.headers.get("x-frame-options"), "DENY");
  assert.equal(response.headers.get("cache-control"), "no-cache");
  assert.match(await response.text(), /home/);
});

test("hashed assets get a long immutable cache", async () => {
  const response = await call("/assets/site.css", {}, { "/assets/site.css": page("body{}", "text/css") });
  assert.equal(response.headers.get("cache-control"), "public, max-age=31536000, immutable");
});

test("sitemap and robots are cached for an hour", async () => {
  for (const path of ["/sitemap.xml", "/robots.txt"]) {
    const response = await call(path, {}, { [path]: page("x", "text/plain") });
    assert.equal(response.headers.get("cache-control"), "public, max-age=3600", path);
  }
});

test("missing pages return the 404 document with a 404 status", async () => {
  const response = await call(
    "/nope/",
    { headers: { Accept: "text/html" } },
    { "/404.html": page("<!doctype html><title>not found</title>") },
  );
  assert.equal(response.status, 404);
  assert.match(await response.text(), /not found/);
  assert.match(response.headers.get("content-type"), /text\/html/);
});

test("missing files that are not pages stay plain 404s", async () => {
  const response = await call("/nope.css", {}, { "/404.html": page("<!doctype html>") });
  assert.equal(response.status, 404);
  assert.equal(await response.text(), "missing");
});

test("writes are refused with Allow", async () => {
  const response = await call("/", { method: "POST" });
  assert.equal(response.status, 405);
  assert.equal(response.headers.get("allow"), "GET, HEAD");
});

test("a missing assets binding is a clear 503", async () => {
  const response = await worker.fetch(new Request("https://mellogram.example/"), {});
  assert.equal(response.status, 503);
});

test("cache policy covers every asset class", () => {
  assert.equal(cacheControl("/assets/x.webp"), "public, max-age=31536000, immutable");
  assert.equal(cacheControl("/download/"), "no-cache");
  assert.equal(cacheControl("/faq/index.html"), "no-cache");
  assert.equal(cacheControl("/favicon.ico"), "public, max-age=86400");
});
