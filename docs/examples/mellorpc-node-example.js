// Minimal MelloRPC v1 client example (Node.js, no third-party packages).
// Windows: uses the MelloGram named pipe.
// macOS/Linux: set MELLOGRAM_CONFIG_DIR to the config directory shown in
// MelloGram → Settings → About & Updates.

"use strict";

const net = require("node:net");
const path = require("node:path");

const endpoint = process.platform === "win32"
  ? String.raw`\\.\pipe\mellorpc-v1`
  : path.join(process.env.MELLOGRAM_CONFIG_DIR || "", "mellorpc.sock");

if (process.platform !== "win32" && !process.env.MELLOGRAM_CONFIG_DIR) {
  throw new Error("Set MELLOGRAM_CONFIG_DIR to MelloGram's config directory.");
}

const clientId = process.env.MELLO_RPC_CLIENT_ID || "com.example.my-game";
const activity = {
  application: "My Game",
  activity: "Exploring the world",
  details: "Chapter 2",
  state: "Playing with friends",
  kind: "playing",
  started_at: Math.floor(Date.now() / 1000),
  ends_at: null,
  party_current: 2,
  party_max: 8,
  join_url: null,
  large_image: "game-world",
  small_image: null
};

const socket = net.createConnection(endpoint);
let pending = Buffer.alloc(0);
let phase = 0;
let heartbeat = null;

function send(message) {
  const body = Buffer.from(JSON.stringify(message), "utf8");
  if (body.length > 1024 * 1024) throw new Error("MelloRPC frame exceeds 1 MiB");
  const header = Buffer.alloc(4);
  header.writeUInt32LE(body.length, 0);
  socket.write(Buffer.concat([header, body]));
}

function sendActivity() {
  send({ type: "set_activity", client_id: clientId, activity, lease_seconds: 120 });
}

socket.on("connect", () => {
  console.log("Connected to MelloGram:", endpoint);
  send({ type: "hello", protocol_version: 1, client_id: clientId, client_name: "Example game" });
});

socket.on("data", (chunk) => {
  pending = Buffer.concat([pending, chunk]);
  while (pending.length >= 4) {
    const length = pending.readUInt32LE(0);
    if (length > 1024 * 1024) {
      socket.destroy(new Error("MelloRPC response exceeded 1 MiB"));
      return;
    }
    if (pending.length < 4 + length) return;
    const response = JSON.parse(pending.subarray(4, 4 + length).toString("utf8"));
    pending = pending.subarray(4 + length);
    if (!response.ok) {
      console.error("MelloRPC rejected request:", response.error);
      socket.destroy();
      return;
    }
    if (phase === 0) {
      phase = 1;
      sendActivity();
    } else if (phase === 1) {
      phase = 2;
      console.log("Presence is active. Refreshing the 120-second lease every 45 seconds.");
      heartbeat = setInterval(sendActivity, 45_000);
    }
  }
});

socket.on("error", (error) => console.error("MelloRPC connection error:", error.message));
socket.on("close", () => {
  if (heartbeat) clearInterval(heartbeat);
  console.log("MelloRPC connection closed; the server clears this client's activity.");
});

function stop() {
  if (heartbeat) clearInterval(heartbeat);
  if (socket.destroyed) return process.exit(0);
  try { send({ type: "clear_activity", client_id: clientId }); } catch {}
  setTimeout(() => socket.end(), 100);
  setTimeout(() => process.exit(0), 500).unref();
}
process.on("SIGINT", stop);
process.on("SIGTERM", stop);
