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

`SITE_URL` sets the public address (default
`https://foulfoxhacks.github.io/MelloGram`). In GitHub, set the repository
variable `SITE_URL` for a custom domain, and enable **Settings → Pages →
Source: GitHub Actions** once.
