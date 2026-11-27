#!/usr/bin/env bash
# Regenerates site data from the real plugin manifests and wallpaper presets.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo run -q -p mellogram-plugins --example catalog | node -e '
const all = JSON.parse(require("fs").readFileSync(0, "utf8"));
const out = all.map((p) => ({ id: p.id, name: p.name, description: p.description, category: p.category,
  default_on: p.enabled, inspired_by: p.inspired_by, permissions: p.permissions, has_ui: p.has_ui,
  settings: p.settings_schema.length }));
process.stdout.write(JSON.stringify(out, null, 2) + "\n");' > site/src/data/plugins.json
cargo run -q -p mellogram-core --example wallpaper_presets | node -e '
const all = JSON.parse(require("fs").readFileSync(0, "utf8"));
process.stdout.write(JSON.stringify(all.map((p) => ({ name: p.name, css: p.css, type: p.wallpaper.kind.type })), null, 2) + "\n");' > site/src/data/wallpapers.json
echo "updated: $(node -e 'console.log(require("./site/src/data/plugins.json").length)') plugins, $(node -e 'console.log(require("./site/src/data/wallpapers.json").length)') wallpapers"
