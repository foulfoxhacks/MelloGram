import { connectWs } from "@fuman/net";
import { FileLocation, IntermediatePacketCodec, MemoryStorage, TelegramClient, tl } from "@mtcute/web";
import qrcode from "qrcode-generator";

const MAX_MEDIA = 4 * 1024 * 1024;

export function relaySocketUrl(page, id) {
  if (!Number.isInteger(id) || id < 1 || id > 5) throw new Error("Unsupported Telegram data centre");
  const url = new URL(`/relay/dc/${id}`, page);
  url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
  return url.href;
}

export function createTelegram(config, page = location.href) {
  const client = new TelegramClient({
    apiId: config.apiId,
    apiHash: config.apiHash,
    storage: new MemoryStorage(),
    logLevel: 0,
    transport: {
      connect: (dc, signal) => connectWs({
        url: relaySocketUrl(page, dc.id),
        implementation: WebSocket,
      }, signal),
      packetCodec: () => new IntermediatePacketCodec(),
    },
  });
  // Never print library errors: they can include details from a Telegram request.
  client.onError.add(() => {});
  return client;
}

export function drawLoginCode(canvas, url) {
  const code = qrcode(0, "M");
  code.addData(url);
  code.make();
  const count = code.getModuleCount();
  const context = canvas.getContext("2d");
  const scale = 6;
  canvas.width = canvas.height = (count + 8) * scale;
  context.fillStyle = "#fff";
  context.fillRect(0, 0, canvas.width, canvas.height);
  context.fillStyle = "#000";
  for (let row = 0; row < count; row++) {
    for (let col = 0; col < count; col++) {
      if (code.isDark(row, col)) context.fillRect((col + 4) * scale, (row + 4) * scale, scale, scale);
    }
  }
}

export function needsPassword(error) {
  return tl.RpcError.is(error, "SESSION_PASSWORD_NEEDED");
}

export function telegramError(error, fallback) {
  for (const [code, message] of [
    ["PHONE_NUMBER_INVALID", "Enter your phone number in international format."],
    ["PHONE_CODE_INVALID", "Telegram rejected that code. Try again."],
    ["PHONE_CODE_EXPIRED", "The code expired. Cancel and request a new code."],
    ["PASSWORD_HASH_INVALID", "Telegram rejected that password. Try again."],
    ["FLOOD_WAIT_%d", "Telegram is limiting requests. Try again later."],
    ["AUTH_KEY_UNREGISTERED", "Your Telegram session ended. Sign out locally, then sign in again."],
  ]) {
    if (tl.RpcError.is(error, code)) return message;
  }
  return fallback;
}

export async function listChats(client) {
  const chats = [];
  for await (const dialog of client.iterDialogs({ limit: 100 })) {
    // Broadcast channels require a separate sponsored message integration.
    if (dialog.peer.type !== "user" && (!["group", "supergroup"].includes(dialog.peer.chatType) || dialog.peer.raw.forum)) continue;
    chats.push(dialog.peer);
  }
  return chats;
}

export function mediaFor(message) {
  // Skip ephemeral and protected content instead of making a copy.
  if (message.raw.noforwards || message.chat.raw.noforwards || message.ttlPeriod) return null;
  const media = message.media;
  if (!(media instanceof FileLocation) || message.raw.media?.ttlSeconds) return null;
  return media;
}

export async function downloadMedia(client, media, signal) {
  if (media.fileSize > MAX_MEDIA) throw new Error("File exceeds the browser download limit");
  const chunks = [];
  let size = 0;
  for await (const chunk of client.downloadAsIterable(media, { abortSignal: signal, limit: MAX_MEDIA + 1 })) {
    signal.throwIfAborted();
    size += chunk.byteLength;
    if (size > MAX_MEDIA) throw new Error("File exceeds the browser download limit");
    chunks.push(chunk);
  }
  if (!size || (media.fileSize != null && size !== media.fileSize)) throw new Error("Incomplete download");
  // Download as opaque bytes so a sender-defined MIME type cannot execute.
  return new Blob(chunks, { type: "application/octet-stream" });
}
