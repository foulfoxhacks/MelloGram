// Renders the live capability list from /api/status. No dependencies, no
// storage: the page re-fetches on load and on visibility changes so it cannot
// show a stale claim about what works.
"use strict";

const list = document.getElementById("capabilities");
const version = document.getElementById("version");
const note = document.getElementById("status-note");

function row(item) {
  const li = document.createElement("li");
  li.dataset.ready = String(Boolean(item.ready));

  const mark = document.createElement("span");
  mark.className = "mark";
  mark.setAttribute("aria-hidden", "true");

  const body = document.createElement("span");
  const label = document.createElement("span");
  label.className = "label";
  label.textContent = item.label;
  const detail = document.createElement("span");
  detail.className = "detail";
  detail.textContent = item.detail;

  body.append(label, detail);
  li.append(mark, body);
  li.setAttribute("aria-label", `${item.label}: ${item.ready ? "available" : "not available yet"}`);
  return li;
}

function render(data) {
  list.replaceChildren(...data.capabilities.map(row));
  version.textContent = `v${data.version} · ${data.capabilities.filter((item) => item.ready).length} of ${data.capabilities.length} ready`;
  note.textContent = "This list is served by the Worker at /api/status, not written into the page.";
}

function failed() {
  list.replaceChildren();
  const li = document.createElement("li");
  li.className = "muted";
  li.textContent = "The status API did not answer. The shell still renders, but check /api/health.";
  list.append(li);
  note.textContent = "";
}

// On wc.<domain> the API lives on wc-api.<domain>; elsewhere (local dev,
// previews) it is same-origin. The same-origin route is the fallback.
const API_BASE = location.protocol === "https:" && location.hostname.startsWith("wc.")
  ? `https://wc-api.${location.hostname.slice(3)}`
  : "";

async function getStatus(base) {
  const response = await fetch(`${base}/api/status`, {
    headers: { Accept: "application/json" },
    credentials: "omit",
    mode: base ? "cors" : "same-origin",
  });
  if (!response.ok) throw new Error(String(response.status));
  return response.json();
}

async function refresh() {
  try {
    render(await getStatus(API_BASE).catch(() => (API_BASE ? getStatus("") : Promise.reject())));
  } catch {
    failed();
  }
}

document.addEventListener("visibilitychange", () => {
  if (document.visibilityState === "visible") refresh();
});

refresh();

// ---- overlay spike ---------------------------------------------------------

const overlayRun = document.getElementById("overlay-run");
const overlayOut = document.getElementById("overlay-out");
const overlayNote = document.getElementById("overlay-note");
const overlayField = (id) => document.getElementById(id);

// Runs one full pairwise exchange with two throwaway identities. Everything is
// in memory: no secret is stored, and leaving the page throws it away.
async function runOverlay() {
  overlayRun.disabled = true;
  overlayNote.textContent = "Loading the WebAssembly module…";
  try {
    const { default: init, WasmIdentity, WasmSession, accept } = await import("./wasm/mellogram_overlay.js");
    await init();
    const alice = WasmIdentity.generate();
    const bob = WasmIdentity.generate();
    const chat = 424242;

    const outbound = WasmSession.start(alice, bob.bundle(), chat);
    const cipher = outbound.encrypt("hello from the browser", chat);
    const accepted = JSON.parse(accept(bob, cipher, chat));
    const state = Uint8Array.from(atob(accepted.state), (c) => c.charCodeAt(0));
    const inbox = WasmSession.fromState(state);
    outbound.decrypt(inbox.encrypt("read you loud and clear", chat), chat);

    overlayField("overlay-safety").textContent = alice.safetyNumber(bob.bundle());
    overlayField("overlay-cipher").textContent = `${cipher.slice(0, 64)}…`;
    overlayField("overlay-roundtrip").textContent = accepted.plaintext;
    overlayOut.hidden = false;
    overlayNote.textContent = outbound.awaitingReply()
      ? "The handshake did not complete — the reply was not accepted."
      : "Both sides agree on the safety number, and the message plus its reply decrypted.";
  } catch (e) {
    overlayNote.textContent = `The overlay module did not load: ${e}`;
  } finally {
    overlayRun.disabled = false;
  }
}

overlayRun?.addEventListener("click", runOverlay);
