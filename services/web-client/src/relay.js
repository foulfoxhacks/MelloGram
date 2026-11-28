// MTProto TCP relay for the browser client.
//
// A browser cannot open a TCP socket and Telegram has no HTTP API, so the web
// client speaks MTProto through this Worker: the page upgrades to a WebSocket
// and the Worker pipes the bytes to one of Telegram's production data centres
// over TCP (`cloudflare:sockets`). This is the transport only. Every byte is
// opaque MTProto: the relay never parses, logs or stores traffic, and it holds
// no Telegram credentials, api_id or session.
//
// It is deliberately not a generic TCP proxy:
//   * the destination comes from a fixed table, chosen by data-centre number.
//     No host, port or URL is ever taken from the client;
//   * browsers must present the web client origin. This prevents cross-site
//     browser use, but does not authenticate non-browser clients;
//   * each connection has a byte and a time budget, and an isolate accepts a
//     small number at once, so one client cannot exhaust the Worker.
//
// The policy below (`relayTarget`, `relayRejection`) is free of Worker-only
// APIs so `node --test` covers it without workerd.

export const DC_ADDRESSES = Object.freeze({
  1: "149.154.175.50",
  2: "149.154.167.51",
  3: "149.154.175.100",
  4: "149.154.167.91",
  5: "91.108.56.130",
});
export const DC_PORT = 443;
/** Bytes relayed per connection, in each direction. */
export const MAX_BYTES = 8 * 1024 * 1024;
/** Wall-clock budget for one connection, in seconds. */
export const MAX_SECONDS = 600;
/** Concurrent relay connections inside one Worker isolate. */
export const MAX_CONNECTIONS = 4;

const originOf = (value) => {
  try {
    return value ? new URL(value).origin : null;
  } catch {
    return null;
  }
};

/** `{ hostname, port }` for a data-centre number, or null when it is not one. */
export function relayTarget(raw) {
  const dc = String(raw ?? "").trim();
  if (!/^[1-5]$/.test(dc)) return null;
  return { hostname: DC_ADDRESSES[dc], port: DC_PORT };
}

/**
 * Why this request may not open a relay, or null when it may.
 * "upgrade-required" means the client did not ask for a WebSocket.
 */
export function relayRejection(request, env) {
  const upgrade = String(request.headers.get("Upgrade") || "").toLowerCase();
  if (request.method !== "GET") return "method";
  if (upgrade !== "websocket") return "upgrade-required";
  const allowed = originOf(env.PUBLIC_BASE_URL);
  if (!allowed || request.headers.get("Origin") !== allowed) return "origin";
  return null;
}

/** The WebSocket URL a page should open for a data centre. */
export function relayUrl(apiOrigin, dc) {
  const url = new URL(apiOrigin);
  url.protocol = url.protocol === "http:" ? "ws:" : "wss:";
  url.pathname = `/relay/dc/${dc}`;
  url.search = "";
  return url.toString();
}

let open = 0;

/**
 * A WebSocket message as bytes. `new Uint8Array(event.data)` is not enough:
 * given anything that is not an ArrayBuffer or a view (a Blob, most notably)
 * it silently builds a zero-length array, and the relay then forwards nothing.
 */
export function bytesOfMessage(data) {
  if (typeof data === "string") return null;
  if (data instanceof ArrayBuffer) return new Uint8Array(data);
  if (ArrayBuffer.isView(data)) return new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
  return null;
}

function plain(message, status, headers = {}) {
  return new Response(message + "\n", {
    status,
    headers: { "Content-Type": "text/plain; charset=utf-8", "Cache-Control": "no-store", ...headers },
  });
}

/**
 * Handles `/relay/dc/<n>`. Returns a 101 upgrade whose WebSocket pipes to
 * Telegram, or a plain-text refusal. `dc` is the path segment already parsed
 * by the caller.
 */
export async function relayFetch(request, env, dc) {
  const target = relayTarget(dc);
  if (!target) return plain("no such data centre", 404);

  const rejection = relayRejection(request, env);
  if (rejection === "method") return plain("method not allowed", 405, { Allow: "GET" });
  if (rejection === "upgrade-required") {
    return plain("this endpoint is a WebSocket relay", 426, { Upgrade: "websocket", Connection: "Upgrade" });
  }
  if (rejection === "origin") return plain("this relay serves the MelloGram web client only", 403);
  const ip = request.headers.get("CF-Connecting-IP");
  if (ip && env.RELAY_RATE_LIMIT) {
    try {
      if (!(await env.RELAY_RATE_LIMIT.limit({ key: ip })).success) {
        return plain("too many relay connections", 429, { "Retry-After": "60" });
      }
    } catch {
      return plain("the relay is temporarily unavailable", 503);
    }
  }

  if (open >= MAX_CONNECTIONS) return plain("the relay is busy, try again in a moment", 503);

  // Reserve before loading the Worker-only module so simultaneous requests
  // cannot all pass the limit while the import is pending.
  open++;
  let sockets;
  let client;
  let server;
  try {
    sockets = await import("cloudflare:sockets");
    [client, server] = Object.values(new WebSocketPair());
    server.binaryType = "arraybuffer";
    server.accept();
  } catch {
    open--;
    return plain("this deployment cannot open TCP sockets", 501);
  }

  let done = false;
  let sent = 0;
  let received = 0;
  let writer = null;
  let socket = null;
  let timer = null;

  const finish = (code, reason) => {
    if (done) return;
    done = true;
    open--;
    if (timer) clearTimeout(timer);
    try {
      server.close(code, reason);
    } catch {
      /* already closed */
    }
    // Close the TCP side too, or the Worker holds the upstream socket until
    // the isolate recycles (a write may still be pending, hence the catch).
    try {
      void socket?.close().catch(() => {});
    } catch {
      /* already closed */
    }
  };

  try {
    socket = sockets.connect({ hostname: target.hostname, port: target.port });
    socket.opened.catch(() => finish(1011, "could not reach telegram"));
    socket.closed.catch(() => finish(1011, "telegram connection lost"));
    writer = socket.writable.getWriter();
  } catch {
    finish(1011, "could not reach telegram");
    return new Response(null, { status: 101, webSocket: client });
  }

  timer = setTimeout(() => finish(1001, "relay time limit reached"), MAX_SECONDS * 1000);

  // WebSocket (browser) -> TCP (Telegram).
  server.addEventListener("message", (event) => {
    if (done) return;
    const chunk = bytesOfMessage(event.data);
    if (!chunk || !chunk.byteLength) {
      finish(1003, "binary frames only");
      return;
    }
    sent += chunk.byteLength;
    if (sent > MAX_BYTES) {
      finish(1009, "relay byte limit reached");
      return;
    }
    writer.write(chunk).catch(() => finish(1011, "telegram connection lost"));
  });
  server.addEventListener("close", () => finish(1000, "closed by client"));
  server.addEventListener("error", () => finish(1011, "relay error"));

  // TCP (Telegram) -> WebSocket (browser).
  (async () => {
    const reader = socket.readable.getReader();
    try {
      for (;;) {
        const { value, done: finished } = await reader.read();
        if (finished) break;
        if (done) break;
        received += value.byteLength;
        if (received > MAX_BYTES) {
          finish(1009, "relay byte limit reached");
          break;
        }
        server.send(value);
      }
      finish(1000, "telegram closed the connection");
    } catch {
      finish(1011, "telegram connection lost");
    } finally {
      reader.releaseLock?.();
    }
  })();

  return new Response(null, { status: 101, webSocket: client });
}
