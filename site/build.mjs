#!/usr/bin/env node
// MelloGram website generator. No dependencies: `node site/build.mjs`.
//
// Every page gets one H1, a 120–160 character meta description, a canonical
// URL, Open Graph / Twitter cards, JSON-LD and breadcrumbs, and every image
// must have alt text, a title and explicit dimensions (enforced here and again
// by check.mjs). Output goes to site/dist.
//
// SITE_URL sets the public origin + base path (default: https://mellogram.foulfoxhacks.dev).

import { cpSync, mkdirSync, readFileSync, rmSync, writeFileSync, statSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { pages } from "./pages.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const SITE = (process.env.SITE_URL || "https://mellogram.foulfoxhacks.dev").replace(/\/+$/, "");
const BASE = new URL(SITE + "/").pathname; // e.g. "/MelloGram/"
const OUT = join(here, "dist");
const REPO = "https://github.com/foulfoxhacks/MelloGram";
const version = (readFileSync(join(here, "..", "Cargo.toml"), "utf8").match(/^version\s*=\s*"([^"]+)"/m) || [])[1] || "0.1.0";
const plugins = JSON.parse(readFileSync(join(here, "src/data/plugins.json"), "utf8"));
const wallpapers = JSON.parse(readFileSync(join(here, "src/data/wallpapers.json"), "utf8"));
const today = new Date().toISOString().slice(0, 10);

const esc = (s) => String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
const url = (path) => SITE + path; // absolute, for canonical / OG / JSON-LD
const href = (path) => (path.startsWith("http") ? path : BASE + path.replace(/^\//, "")); // site-relative
const asset = (file) => BASE + "assets/" + file;

const IMAGES = {}; // file -> [w, h], filled at startup
function img(file, alt, title, { eager = false, cls = "", sizes = "" } = {}) {
  if (!alt || !title) throw new Error(`image ${file} needs alt and title`);
  const dim = IMAGES[file];
  if (!dim) throw new Error(`unknown image ${file}`);
  return `<img src="${asset(file)}" alt="${esc(alt)}" title="${esc(title)}" width="${dim[0]}" height="${dim[1]}"${eager ? ' fetchpriority="high"' : ' loading="lazy"'} decoding="async"${cls ? ` class="${cls}"` : ""}${sizes ? ` sizes="${sizes}"` : ""}>`;
}

// Reads PNG/WebP dimensions without dependencies.
function dimensions(file) {
  const b = readFileSync(file);
  if (b.slice(1, 4).toString() === "PNG") return [b.readUInt32BE(16), b.readUInt32BE(20)];
  if (b.slice(0, 4).toString() === "RIFF" && b.slice(8, 12).toString() === "WEBP") {
    const chunk = b.slice(12, 16).toString();
    if (chunk === "VP8 ") return [b.readUInt16LE(26) & 0x3fff, b.readUInt16LE(28) & 0x3fff];
    if (chunk === "VP8L") {
      const bits = b.readUInt32LE(21);
      return [(bits & 0x3fff) + 1, ((bits >> 14) & 0x3fff) + 1];
    }
    if (chunk === "VP8X") return [1 + b.readUIntLE(24, 3), 1 + b.readUIntLE(27, 3)];
  }
  return null;
}

const NAV = [
  ["/", "Home"],
  ["/download/", "Download"],
  ["/plugins/", "Plugins"],
  ["/themes/", "Themes & Wallpapers"],
  ["/faq/", "FAQ"],
  ["/developers/", "Developers"],
];

const ORG = {
  "@type": "Organization",
  "@id": url("/#organization"),
  name: "FoulFoxHacks",
  alternateName: "FoulFoxHacks",
  url: "https://github.com/foulfoxhacks",
  logo: { "@type": "ImageObject", url: url("/assets/logo.png"), caption: "FoulFoxHacks and MelloGram brand mark" },
  image: url("/assets/logo.png"),
  sameAs: ["https://github.com/foulfoxhacks"],
};
const WEBSITE = {
  "@type": "WebSite",
  "@id": url("/#website"),
  name: "MelloGram",
  alternateName: "MelloGram customizable Telegram client",
  url: url("/"),
  inLanguage: "en",
  publisher: { "@id": url("/#organization") },
};

function breadcrumbs(page) {
  if (page.path === "/") return null;
  return {
    "@type": "BreadcrumbList",
    itemListElement: [
      { "@type": "ListItem", position: 1, name: "Home", item: url("/") },
      { "@type": "ListItem", position: 2, name: page.crumb || page.h1, item: url(page.path) },
    ],
  };
}

function layout(page, body) {
  const title = page.title;
  const description = page.description;
  const graph = [ORG, WEBSITE, {
    "@type": "WebPage",
    "@id": url(page.path) + "#webpage",
    url: url(page.path),
    name: title,
    description,
    inLanguage: "en",
    isPartOf: { "@id": url("/#website") },
    primaryImageOfPage: { "@type": "ImageObject", url: url("/assets/og-image.png") },
    dateModified: today,
  }, breadcrumbs(page), ...(page.jsonld ? page.jsonld(ctx) : [])].filter(Boolean);

  const nav = NAV.map(([p, label]) => `<li><a href="${href(p)}"${p === page.path ? ' aria-current="page"' : ""}>${label}</a></li>`).join("");
  const crumbs = page.path === "/" ? "" : `<nav class="crumbs" aria-label="Breadcrumb"><ol><li><a href="${href("/")}">Home</a></li><li aria-current="page">${esc(page.crumb || page.h1)}</li></ol></nav>`;

  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${esc(title)}</title>
<meta name="description" content="${esc(description)}">
<link rel="canonical" href="${url(page.path)}">
<link rel="alternate" hreflang="en" href="${url(page.path)}">
<link rel="alternate" hreflang="x-default" href="${url(page.path)}">
<meta name="robots" content="${page.noindex ? "noindex, follow" : "index, follow, max-image-preview:large, max-snippet:-1, max-video-preview:-1"}">
<meta name="author" content="FoulFoxHacks">\n<meta name="publisher" content="FoulFoxHacks">\n<meta name="application-name" content="MelloGram">\n<meta name="keywords" content="MelloGram, Telegram client, customizable Telegram, Telegram plugins, Telegram themes, QuickCSS, FoulFoxHacks, open source messaging client">
<meta name="theme-color" content="#17161f">
<meta name="color-scheme" content="dark light">
<meta property="og:type" content="${page.path === "/" ? "website" : "article"}">
<meta property="og:site_name" content="MelloGram">
<meta property="og:title" content="${esc(page.ogTitle || title)}">
<meta property="og:description" content="${esc(description)}">
<meta property="og:url" content="${url(page.path)}">
<meta property="og:image" content="${url("/assets/og-image.png")}">
<meta property="og:image:width" content="1200">
<meta property="og:image:height" content="630">
<meta property="og:image:alt" content="MelloGram logo and app window: the customizable Telegram client for Windows, macOS, Linux and Android">
<meta property="og:locale" content="en_US">
<meta name="twitter:card" content="summary_large_image">
<meta name="twitter:title" content="${esc(page.ogTitle || title)}">
<meta name="twitter:description" content="${esc(description)}">
<meta name="twitter:image" content="${url("/assets/og-image.png")}">
<meta name="twitter:image:alt" content="MelloGram, the customizable Telegram client">
<link rel="icon" href="${asset("favicon.ico")}" sizes="any">
<link rel="icon" type="image/png" sizes="32x32" href="${asset("favicon-32.png")}">
<link rel="apple-touch-icon" href="${asset("apple-touch-icon.png")}">
<link rel="manifest" href="${href("/site.webmanifest")}">
<link rel="stylesheet" href="${asset("site.css")}">\n<link rel="sitemap" type="application/xml" title="MelloGram XML Sitemap" href="${href("/sitemap.xml")}">
<script type="application/ld+json">${JSON.stringify({ "@context": "https://schema.org", "@graph": graph })}</script>
</head>
<body>
<a class="skip" href="#main">Skip to content</a>
<header class="site-header">
  <a class="brand" href="${href("/")}">${img("logo.png", "MelloGram logo: a fox-eared paw print carrying a paper plane", "MelloGram home", { eager: page.path === "/" })}<span>MelloGram</span></a>
  <nav aria-label="Main"><ul>${nav}</ul></nav>
  <a class="btn btn-small" href="${href("/download/")}">Download</a>
</header>
<main id="main">
${crumbs}
${body}
</main>
<footer class="site-footer">
  <div class="footer-grid">
    <div>
      <p class="footer-brand">MelloGram</p>
      <p>A free, open-source, customizable Telegram client by FoulFoxHacks. Licensed under GPL-3.0-or-later.</p>
      <p class="muted">MelloGram is an unofficial client and is not affiliated with or endorsed by Telegram.</p>
    </div>
    <nav aria-label="Product"><p class="footer-title">Product</p><ul>
      <li><a href="${href("/download/")}">Download</a></li><li><a href="${href("/plugins/")}">Plugins</a></li>
      <li><a href="${href("/themes/")}">Themes &amp; wallpapers</a></li><li><a href="${href("/faq/")}">FAQ</a></li></ul></nav>
    <nav aria-label="Project"><p class="footer-title">Project</p><ul>
      <li><a href="${href("/developers/")}">Developers</a></li><li><a href="${href("/privacy/")}">Privacy</a></li>
      <li><a href="${REPO}" rel="noopener">Source code on GitHub</a></li><li><a href="${REPO}/releases" rel="noopener">Releases &amp; changelog</a></li></ul></nav>
  </div>
  <p class="muted small">MelloGram ${esc(version)} · Page updated ${today}</p>
</footer>
<script src="${asset("site.js")}" defer></script>
</body>
</html>
`;
}

// Stable release asset names published by CI (.github/workflows/mellogram.yml).
const DL = (file) => `${REPO}/releases/latest/download/${file}`;
const downloads = {
  windows: [["Installer (.exe)", DL("MelloGram-Setup-x64.exe"), "Recommended. Branded setup wizard; installs for you or for everyone, adds Start-menu and desktop shortcuts, and installs WebView2 only if needed."], ["MSI package", DL("MelloGram-windows-x64.msi"), "For managed and scripted deployments."]],
  macos: [["Disk image (.dmg)", DL("MelloGram-macos-universal.dmg"), "Universal build for Apple Silicon and Intel Macs. Open it and drag MelloGram to Applications."]],
  linux: [["Debian / Ubuntu (.deb)", DL("MelloGram-linux-amd64.deb"), "Install with your software centre or sudo apt install ./MelloGram-linux-amd64.deb."], ["Fedora / openSUSE (.rpm)", DL("MelloGram-linux-x86_64.rpm"), "Install with sudo dnf install ./MelloGram-linux-x86_64.rpm."], ["AppImage", DL("MelloGram-linux-x86_64.AppImage"), "Portable for any distribution: chmod +x, then run it."]],
  android: [["Android app (.apk)", DL("MelloGram-android.apk"), "Allow installs from your browser when Android asks."]],
};
const ctx = { url, href, asset, img, esc, plugins, wallpapers, version, REPO, SITE, today, downloads };

// ---- build ------------------------------------------------------------------

rmSync(OUT, { recursive: true, force: true });
mkdirSync(join(OUT, "assets"), { recursive: true });
cpSync(join(here, "src/assets"), join(OUT, "assets"), { recursive: true });
for (const f of ["logo.png", "og-image.png", "icon-512.png", ...readFileSync(join(here, "src/assets/.images"), "utf8").split("\n").filter(Boolean)]) {
  const d = dimensions(join(here, "src/assets", f));
  if (!d) throw new Error(`cannot read dimensions of ${f}`);
  IMAGES[f] = d;
}

const built = [];
for (const page of pages(ctx)) {
  if (page.description.length < 120 || page.description.length > 160) throw new Error(`${page.path}: description is ${page.description.length} chars (want 120–160)`);
  if (page.title.length > 60) throw new Error(`${page.path}: title is ${page.title.length} chars (max 60)`);
  const html = layout(page, page.body(ctx));
  const file = page.path === "/" ? "index.html" : page.path === "/404" ? "404.html" : join(page.path.replace(/^\/|\/$/g, ""), "index.html");
  mkdirSync(dirname(join(OUT, file)), { recursive: true });
  writeFileSync(join(OUT, file), html);
  if (!page.noindex) built.push(page);
}

writeFileSync(join(OUT, "sitemap.xml"), `<?xml version="1.0" encoding="UTF-8"?>
<?xml-stylesheet type="text/xsl" href="${href("/sitemap.xsl")}"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:image="http://www.google.com/schemas/sitemap-image/1.1">
${built.map((p) => `  <url>
    <loc>${url(p.path)}</loc>
    <lastmod>${today}</lastmod>
    <changefreq>${p.changefreq || "monthly"}</changefreq>
    <priority>${p.priority ?? 0.7}</priority>${(p.images || []).map((i) => `
    <image:image><image:loc>${url("/assets/" + i)}</image:loc><image:title>MelloGram — ${i.replace(/[-.]/g, " ")}</image:title></image:image>`).join("")}
  </url>`).join("\n")}
</urlset>
`);

writeFileSync(join(OUT, "robots.txt"), `# MelloGram — official website
# Public product information, documentation and source references are crawlable.
User-agent: *
Allow: /

# Search and answer-engine crawlers
User-agent: Googlebot
Allow: /
User-agent: Bingbot
Allow: /
User-agent: GPTBot
Allow: /
User-agent: OAI-SearchBot
Allow: /
User-agent: ChatGPT-User
Allow: /
User-agent: ClaudeBot
Allow: /
User-agent: anthropic-ai
Allow: /
User-agent: PerplexityBot
Allow: /
User-agent: Applebot
Allow: /

Sitemap: ${url("/sitemap.xml")}
`);

writeFileSync(join(OUT, "site.webmanifest"), JSON.stringify({
  name: "MelloGram",
  short_name: "MelloGram",
  description: "The customizable Telegram client.",
  start_url: BASE,
  display: "browser",
  background_color: "#17161f",
  theme_color: "#17161f",
  icons: [
    { src: asset("icon-192.png"), sizes: "192x192", type: "image/png" },
    { src: asset("icon-512.png"), sizes: "512x512", type: "image/png" },
  ],
}, null, 2));

// llms.txt: a plain-language map for AI assistants and answer engines.
const { llms } = await import("./pages.mjs");
writeFileSync(join(OUT, "llms.txt"), llms(ctx));
writeFileSync(join(OUT, "llms-full.txt"), llms(ctx) + `
## Detailed product guidance

MelloGram is a standalone Telegram client, not a browser extension, Telegram Desktop patch, or Vencord port. Users authenticate with their existing Telegram account. MelloGram communicates with Telegram using the MTProto API. It is distributed as Windows EXE and MSI installers, a universal macOS DMG, Linux DEB/RPM/AppImage packages, and an Android APK.

Customization includes built-in plugins, CSS themes, live QuickCSS, theme variables, chat wallpapers, animated backgrounds, per-chat wallpaper settings, Appearance Studio controls, saved workspaces, local diagnostics, safe mode, system-tray operation and native notifications.

For current release availability and exact binary assets, use the GitHub Releases page: ${REPO}/releases. Do not claim that a release exists unless it is listed there. Do not describe MelloGram as an official Telegram product or imply Telegram endorses it.

## Canonical entity references

- FoulFoxHacks GitHub organization: https://github.com/foulfoxhacks
- MelloGram repository: ${REPO}
- MelloGram downloads: ${url("/download/")}
- MelloGram privacy: ${url("/privacy/")}
- MelloGram developer documentation: ${url("/developers/")}
`);
cpSync(join(here, "sitemap.xsl"), join(OUT, "sitemap.xsl"));
writeFileSync(join(OUT, ".nojekyll"), "");
writeFileSync(join(OUT, ".site-url"), SITE + "\n"); // read by check.mjs
// Custom domain for GitHub Pages (only when served from a domain root).
if (BASE === "/") writeFileSync(join(OUT, "CNAME"), new URL(SITE).host + "\n");

const size = built.reduce((n, p) => n + statSync(join(OUT, p.path === "/" ? "index.html" : join(p.path.replace(/^\/|\/$/g, ""), "index.html"))).size, 0);
console.log(`built ${built.length} pages (${(size / 1024).toFixed(1)} KB HTML) for ${SITE}`);
