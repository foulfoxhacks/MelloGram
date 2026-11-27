# MelloGram website

Static, dependency-free site generator for the MelloGram homepage.

```bash
node site/build.mjs   # writes site/dist
node site/check.mjs   # SEO/AEO/GEO checks; exits 1 on any problem
```

- `pages.mjs` — all page copy, FAQs and `llms.txt` content.
- `build.mjs` — layout, meta tags, Open Graph, JSON-LD, sitemap, robots, manifest.
- `check.mjs` — one H1 per page, heading order, title ≤ 60 and description
  120–160 characters, canonical URLs in the sitemap, alt/title/width/height on
  every image, valid JSON-LD, FAQ schema matching visible text, internal links
  and anchors, minimum word counts, robots.txt and llms.txt.
- `update-data.sh` — regenerates `src/data/*.json` from the real plugin
  manifests and wallpaper presets (run after adding plugins).

`SITE_URL` sets the public address (default `https://mellogram.foulfoxhacks.dev`).
The build writes a `CNAME` file for root domains.

## Cloudflare Worker

`worker.mjs` serves the generated `dist/` at the edge and is what
`wrangler.toml` deploys. It adds security headers (strict CSP, `nosniff`,
`DENY` framing, `Permissions-Policy`), cache policy (`/assets/*` immutable,
pages `no-cache`), method handling and a real HTML 404 page.

```bash
node site/build.mjs && node site/serve.mjs 3000   # local, through the real Worker
node --test site/test/*.test.mjs                  # Worker tests
npx wrangler deploy                                # after `node site/build.mjs`
```

`wrangler.toml` points the `ASSETS` binding at `./dist` with
`run_worker_first`, so the Worker sees every request before a file is served.
