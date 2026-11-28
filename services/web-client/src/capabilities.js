// Shared metadata for the browser client. Kept free of Worker-only APIs so the
// same module runs under `node --test`.

import { telegramConfig } from "./telegram-config.js";

export const SERVICE = "mellogram-web-client";
export const VERSION = "0.2.0";

/**
 * What the browser client can honestly do today. Every entry says what is
 * missing rather than pretending a feature exists: an entry flips to true only
 * when the feature is implemented and tested in this service.
 */
export function capabilities(beta = false) {
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
      detail: "The Worker pipes the page's MTProto bytes to a fixed Telegram data centre over TCP (/relay/dc/1-5). Fixed destinations with a browser origin check, byte and time limits.",
    },
    {
      id: "login",
      label: "Telegram sign-in",
      ready: false,
      beta,
      detail: beta ? "QR and phone sign-in are available for beta testing." : "Sign-in is disabled on this deployment. Use MelloGram desktop or Android.",
    },
    {
      id: "messaging",
      label: "Chats, messages, media",
      ready: false,
      beta,
      detail: beta ? "The beta supports recent chats, text messages and small downloads." : "Available only when the messaging beta is enabled. The Worker does not store messages.",
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
  const beta = Boolean(telegramConfig(env));
  return {
    service: SERVICE,
    version: VERSION,
    stage: beta ? "beta" : "transport",
    telegramBeta: beta,
    origin: env.PUBLIC_BASE_URL || null,
    api: env.API_BASE_URL || null,
    capabilities: capabilities(beta),
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
