// Entry point for wrangler.embedded.toml: the normal Worker, with the static
// shell served from files bundled into the script instead of the ASSETS binding.
import worker from "./worker.js";
import files from "./embedded-assets.js";

const decode = (b64) => Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));

const embeddedAssets = {
  async fetch(request) {
    let path = new URL(request.url).pathname;
    if (path.endsWith("/")) path += "index.html";
    const file = files[path];
    if (!file) return new Response("Not found\n", { status: 404, headers: { "Content-Type": "text/plain; charset=utf-8" } });
    const headers = { "Content-Type": file.type, "Cache-Control": path.endsWith(".html") ? "no-cache" : "public, max-age=3600" };
    return new Response(request.method === "HEAD" ? null : decode(file.b64), { headers });
  },
};

export default {
  fetch(request, env, ctx) {
    return worker.fetch(request, { ...env, ASSETS: env.ASSETS || embeddedAssets }, ctx);
  },
};
