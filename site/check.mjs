#!/usr/bin/env node
// SEO / AEO / GEO checks for site/dist. Fails (exit 1) on any problem.
// Run after build: `node site/build.mjs && node site/check.mjs`.

import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const DIST = join(here, "dist");
// Use exactly the URL the build used, so build and check can never disagree.
const SITE = readFileSync(join(DIST, ".site-url"), "utf8").trim().replace(/\/+$/, "");
const BASE = new URL(SITE + "/").pathname;
const problems = [];
const report = [];
const fail = (page, msg) => problems.push(`${page}: ${msg}`);

function walk(dir) {
  return readdirSync(dir).flatMap((f) => {
    const p = join(dir, f);
    return statSync(p).isDirectory() ? walk(p) : [p];
  });
}

const attr = (tag, name) => (tag.match(new RegExp(`\\s${name}="([^"]*)"`, "i")) || [])[1];
const text = (html) => html.replace(/<script[\s\S]*?<\/script>/gi, " ").replace(/<style[\s\S]*?<\/style>/gi, " ").replace(/<[^>]+>/g, " ").replace(/&[a-z#0-9]+;/gi, " ").replace(/\s+/g, " ").trim();

const files = walk(DIST);
const pages = files.filter((f) => f.endsWith(".html"));
const sitemap = existsSync(join(DIST, "sitemap.xml")) ? readFileSync(join(DIST, "sitemap.xml"), "utf8") : "";
const sitemapLocs = [...sitemap.matchAll(/<loc>([^<]+)<\/loc>/g)].map((m) => m[1]);

for (const file of pages) {
  const rel = "/" + relative(DIST, file).replace(/index\.html$/, "").replace(/\\/g, "/");
  const html = readFileSync(file, "utf8");
  const is404 = rel === "/404.html";

  if (!/<html lang="en">/.test(html)) fail(rel, "missing <html lang>");
  if (!/<meta name="viewport"/.test(html)) fail(rel, "missing viewport");

  const title = (html.match(/<title>([^<]*)<\/title>/) || [])[1];
  if (!title) fail(rel, "missing <title>");
  else if (title.length > 60) fail(rel, `title too long (${title.length})`);

  const desc = (html.match(/<meta name="description" content="([^"]*)"/) || [])[1];
  if (!desc) fail(rel, "missing meta description");
  else if (desc.length < 120 || desc.length > 160) fail(rel, `description length ${desc.length} (want 120–160)`);

  const canonical = (html.match(/<link rel="canonical" href="([^"]+)"/) || [])[1];
  if (!canonical || !canonical.startsWith("https://")) fail(rel, "canonical missing or not absolute https");
  if (!is404 && canonical && !sitemapLocs.includes(canonical)) fail(rel, `canonical ${canonical} not in sitemap`);
  for (const p of ["og:title", "og:description", "og:image", "og:url", "og:type"]) {
    if (!html.includes(`property="${p}"`)) fail(rel, `missing ${p}`);
  }
  if (!html.includes('name="twitter:card"')) fail(rel, "missing twitter:card");
  if (/name="robots" content="[^"]*noindex/.test(html) && !is404) fail(rel, "page is noindex");

  // Headings: exactly one H1, no skipped levels.
  const headings = [...html.matchAll(/<h([1-6])[\s>]/g)].map((m) => Number(m[1]));
  const h1s = headings.filter((h) => h === 1).length;
  if (h1s !== 1) fail(rel, `has ${h1s} <h1> (want exactly 1)`);
  for (let i = 1; i < headings.length; i++) {
    if (headings[i] > headings[i - 1] + 1) fail(rel, `heading level jumps from h${headings[i - 1]} to h${headings[i]}`);
  }

  // Images: alt, title, width, height.
  for (const tag of html.match(/<img\b[^>]*>/g) || []) {
    const src = attr(tag, "src");
    for (const a of ["alt", "title", "width", "height"]) {
      const v = attr(tag, a);
      if (!v || !v.trim()) fail(rel, `<img src="${src}"> missing ${a}`);
    }
    if (src && src.startsWith(BASE) && !existsSync(join(DIST, src.slice(BASE.length)))) fail(rel, `image not found: ${src}`);
  }

  // JSON-LD parses and uses schema.org.
  const ld = [...html.matchAll(/<script type="application\/ld\+json">([\s\S]*?)<\/script>/g)];
  if (!ld.length) fail(rel, "no JSON-LD");
  const types = [];
  for (const m of ld) {
    try {
      const data = JSON.parse(m[1]);
      if (data["@context"] !== "https://schema.org") fail(rel, "JSON-LD @context is not https://schema.org");
      for (const node of data["@graph"] || [data]) types.push(node["@type"]);
      for (const node of data["@graph"] || []) {
        if (node["@type"] === "FAQPage") {
          for (const q of node.mainEntity) {
            if (!text(html).includes(q.name)) fail(rel, `FAQ question not visible on page: ${q.name}`);
          }
        }
      }
    } catch (e) {
      fail(rel, `invalid JSON-LD: ${e.message}`);
    }
  }

  // Internal links resolve (files and #anchors).
  for (const m of html.matchAll(/<a\b[^>]*href="([^"]+)"/g)) {
    const link = m[1];
    if (/^(https?:|mailto:)/.test(link)) continue;
    const [path, hash] = link.split("#");
    let target = file;
    if (path) {
      if (!path.startsWith(BASE)) { fail(rel, `link outside base: ${link}`); continue; }
      const local = path.slice(BASE.length);
      target = join(DIST, local, local === "" || local.endsWith("/") ? "index.html" : "");
      if (!existsSync(target)) { fail(rel, `broken link: ${link}`); continue; }
    }
    if (hash && !readFileSync(target, "utf8").includes(`id="${hash}"`)) fail(rel, `missing anchor: ${link}`);
  }

  const words = text(html.split("<main")[1] || "").split(" ").length;
  if (!is404 && words < 250) fail(rel, `thin content: ${words} words`);
  const imgs = (html.match(/<img\b/g) || []).length;
  report.push(`${rel.padEnd(14)} ${String(words).padStart(5)} words  h1=${h1s}  imgs=${imgs}  ld=[${[...new Set(types)].join(", ")}]  title=${title?.length}  desc=${desc?.length}`);
}

// Site-wide files.
const robots = existsSync(join(DIST, "robots.txt")) ? readFileSync(join(DIST, "robots.txt"), "utf8") : "";
if (!robots.includes(`Sitemap: ${SITE}/sitemap.xml`)) problems.push("robots.txt: missing Sitemap line");
if (/Disallow:\s*\/\s*$/m.test(robots)) problems.push("robots.txt: disallows everything");
if (!existsSync(join(DIST, "llms.txt"))) problems.push("llms.txt missing");
if (!existsSync(join(DIST, "llms-full.txt"))) problems.push("llms-full.txt missing");
if (!existsSync(join(DIST, "assets", "sitemap.css"))) problems.push("CSS sitemap stylesheet missing");
else {
  const css = readFileSync(join(DIST, "assets", "sitemap.css"), "utf8");
  if (!css.includes("#79baff") || !css.includes("urlset") || !css.includes("/assets/logo.png") ||
      !css.includes('@namespace url("http://www.sitemaps.org/schemas/sitemap/0.9")') ||
      !css.includes('@namespace image url("http://www.google.com/schemas/sitemap-image/1.1")')) {
    problems.push("sitemap.css is missing MelloGram styling, logo or XML namespace selectors");
  }
}
if (!sitemap.includes('<?xml-stylesheet type="text/css" href="/assets/sitemap.css"?>')) problems.push("sitemap.xml missing CSS stylesheet reference");
if (sitemap.includes("text/xsl") || existsSync(join(DIST, "sitemap.xsl"))) problems.push("deprecated XSL sitemap stylesheet is still present");
if (sitemapLocs.some((l) => l.includes("404"))) problems.push("sitemap lists the 404 page");
for (const loc of sitemapLocs) {
  if (!loc.startsWith(SITE)) problems.push(`sitemap loc outside site: ${loc}`);
}

console.log(report.join("\n"));
if (problems.length) {
  console.error(`\n${problems.length} problem(s):\n- ` + problems.join("\n- "));
  process.exit(1);
}
console.log(`\nAll checks passed: ${pages.length} pages, ${sitemapLocs.length} sitemap URLs.`);
