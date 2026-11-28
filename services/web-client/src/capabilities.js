// Shared metadata for the browser client. Kept free of Worker-only APIs so the
// same module runs under `node --test`.

export const SERVICE = "mellogram-web-client";
export const VERSION = "0.1.0";

/**
 * What the browser client can honestly do today. Every entry says what is
 * missing rather than pretending a feature exists: an entry flips to true only
 * when the feature is implemented and tested in this service.
 */
export function capabilities() {
  return [
    {
      id: "shell",
      label: "Web shell and status API",
      ready: true,
      detail: "This page, /api/health and /api/status.",
    },
    {
      id: "relay",
      label: "MTProto transport relay",
      ready: true,
      detail: "The Worker pipes the page's MTProto bytes to a fixed Telegram data centre over TCP (/relay/dc/1-5). No client-chosen host or port, own origin only, byte and time limits.",
    },
    {
      id: "login",
      label: "Telegram sign-in",
      ready: false,
      detail: "The transport is in place; the key exchange, QR login and message encryption are the next step. The desktop app owns account login today.",
    },
    {
      id: "messaging",
      label: "Chats, messages, media",
      ready: false,
      detail: "Needs the sign-in handshake first. Nothing is stored server-side until then.",
    },
    {
      id: "overlay-wasm",
      label: "Overlay protocol in the browser",
      ready: true,
      detail: "The Rust protocol runs as WebAssembly: identities, safety numbers and pairwise sessions, in memory only.",
    },
    {
      id: "overlay",
      label: "Overlay in chats",
      ready: false,
      detail: "Deciding where secrets live in the browser comes first; nothing is persisted or wired into messaging yet.",
    },
  ];
}

export function status(env = {}) {
  return {
    service: SERVICE,
    version: VERSION,
    stage: "transport",
    origin: env.PUBLIC_BASE_URL || null,
    api: env.API_BASE_URL || null,
    capabilities: capabilities(),
  };
}

export function json(data, statusCode = 200, headers = {}) {
  return new Response(JSON.stringify(data), {
    status: statusCode,
    headers: {
      "Content-Type": "application/json; charset=utf-8",
      "Cache-Control": "no-store",
      ...headers,
    },
  });
}

export function errorJson(message, statusCode) {
  return json({ error: message }, statusCode);
}
