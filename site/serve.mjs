// Local preview without Wrangler: build the site, then run this.
//
//   node site/build.mjs && node site/serve.mjs [port]
//
// It runs the real Worker from site/worker.mjs over `dist/`, so the security
// headers, cache policy and HTML 404 are the ones that ship.
import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { extname, join, normalize } from "node:path";
import { fileURLToPath } from "node:url";

import worker from "./worker.mjs";

const here = fileURLToPath(new URL(".", import.meta.url));
const root = join(here, "dist");
const port = Number(process.argv[2] || 8788);
const types = {
  ".html": "text/html; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".xml": "application/xml; charset=utf-8",
  ".txt": "text/plain; charset=utf-8",
  ".webmanifest": "application/manifest+json",
  ".png": "image/png",
  ".webp": "image/webp",
  ".ico": "image/x-icon",
  ".svg": "image/svg+xml",
};

async function asset(request) {
  const path = new URL(request.url).pathname;
  const file = normalize(join(root, path === "/" ? "index.html" : path));
  if (!file.startsWith(root)) return new Response("not found", { status: 404 });
  try {
    const body = await readFile(file);
    return new Response(body, { headers: { "Content-Type": types[extname(file)] || "application/octet-stream" } });
  } catch {
    return new Response("not found", { status: 404 });
  }
}

const env = { ASSETS: { fetch: asset } };

createServer(async (req, res) => {
  const origin = `http://${req.headers.host || `127.0.0.1:${port}`}`;
  const request = new Request(new URL(req.url, origin), { method: req.method, headers: req.headers });
  const response = await worker.fetch(request, env);
  res.writeHead(response.status, Object.fromEntries(response.headers));
  res.end(Buffer.from(await response.arrayBuffer()));
}).listen(port, "0.0.0.0", () => {
  console.log(`MelloGram site (Worker) on http://0.0.0.0:${port}`);
});
