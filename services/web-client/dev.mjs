// Local preview without Wrangler: runs the real Worker module through a tiny
// node:http adapter and serves the shell from public/. Useful for checking the
// page and /api/status on a machine that cannot install Wrangler.
//
//   node dev.mjs [port]        # default 8787
//
// This is a development convenience, not the deployment path: Wrangler owns
// bindings, headers and asset routing in production.
import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { extname, join, normalize } from "node:path";
import { fileURLToPath } from "node:url";

import worker from "./src/worker.js";

const here = fileURLToPath(new URL(".", import.meta.url));
const port = Number(process.argv[2] || 8787);
const types = {
  ".html": "text/html; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".svg": "image/svg+xml",
  ".png": "image/png",
  ".wasm": "application/wasm",
  ".ico": "image/x-icon",
};

async function asset(request) {
  const path = new URL(request.url).pathname;
  const file = normalize(join(here, "public", path === "/" ? "index.html" : path));
  if (!file.startsWith(join(here, "public"))) {
    return new Response("not found", { status: 404 });
  }
  try {
    const body = await readFile(file);
    return new Response(body, {
      headers: { "Content-Type": types[extname(file)] || "application/octet-stream" },
    });
  } catch {
    return new Response("not found", { status: 404 });
  }
}

const env = {
  PUBLIC_BASE_URL: `http://127.0.0.1:${port}`,
  ASSETS: { fetch: asset },
};

createServer(async (req, res) => {
  const origin = `http://${req.headers.host || `127.0.0.1:${port}`}`;
  const chunks = [];
  for await (const chunk of req) chunks.push(chunk);
  const request = new Request(new URL(req.url, origin), {
    method: req.method,
    headers: req.headers,
    body: chunks.length ? Buffer.concat(chunks) : undefined,
  });
  const response = await worker.fetch(request, env);
  res.writeHead(response.status, Object.fromEntries(response.headers));
  res.end(Buffer.from(await response.arrayBuffer()));
}).listen(port, "0.0.0.0", () => {
  console.log(`MelloGram web client on http://0.0.0.0:${port}`);
});
