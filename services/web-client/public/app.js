// Renders the live capability list from /api/status. No dependencies, no
// storage: the page re-fetches on load and on visibility changes so it cannot
// show a stale claim about what works.
import { setLoginAvailability } from "./signin.js";

const list = document.getElementById("capabilities");
const version = document.getElementById("version");
const stage = document.getElementById("stage");
const note = document.getElementById("status-note");

function row(item) {
  const li = document.createElement("li");
  li.dataset.ready = item.beta ? "beta" : String(item.ready);

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
  li.setAttribute("aria-label", `${item.label}: ${item.ready ? "available" : item.beta ? "beta" : "unavailable"}`);
  return li;
}

function render(data) {
  list.replaceChildren(...data.capabilities.map(row));
  setLoginAvailability(data.telegramBeta === true);
  version.textContent = `v${data.version} · ${data.capabilities.filter((item) => item.ready).length} of ${data.capabilities.length} ready`;
  if (data.stage) stage.textContent = data.stage;
  note.textContent = "Status provided by /api/status.";
}

function failed() {
  list.replaceChildren();
  const li = document.createElement("li");
  li.className = "muted";
  li.textContent = "The status API did not answer. The shell still renders, but check /api/health.";
  list.append(li);
  note.textContent = "";
}

let refreshId = 0;
async function refresh() {
  const id = ++refreshId;
  try {
    const response = await fetch("/api/status", {
      headers: { Accept: "application/json" },
      credentials: "omit",
      cache: "no-store",
      signal: AbortSignal.timeout(8000),
    });
    if (!response.ok) throw new Error("Status unavailable");
    const data = await response.json();
    if (id === refreshId) render(data);
  } catch {
    if (id !== refreshId) return;
    version.textContent = "";
    stage.textContent = "status unavailable";
    setLoginAvailability(false, true);
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
  overlayOut.hidden = true;
  const allocations = [];
  overlayNote.textContent = "Loading the WebAssembly module…";
  try {
    const { default: init, WasmIdentity, WasmSession, accept } = await import("./wasm/mellogram_overlay.js");
    await init();
    const alice = WasmIdentity.generate();
    allocations.push(alice);
    const bob = WasmIdentity.generate();
    allocations.push(bob);
    const chat = 424242;

    const outbound = WasmSession.start(alice, bob.bundle(), chat);
    allocations.push(outbound);
    const cipher = outbound.encrypt("hello from the browser", chat);
    const accepted = JSON.parse(accept(bob, cipher, chat));
    const state = Uint8Array.from(atob(accepted.state), (c) => c.charCodeAt(0));
    let inbox;
    try { inbox = WasmSession.fromState(state); }
    finally { state.fill(0); }
    allocations.push(inbox);
    const reply = outbound.decrypt(inbox.encrypt("read you loud and clear", chat), chat);
    const safety = alice.safetyNumber(bob.bundle());
    if (accepted.plaintext !== "hello from the browser" || reply !== "read you loud and clear"
        || outbound.awaitingReply() || safety !== bob.safetyNumber(alice.bundle())) {
      throw new Error("The exchange did not verify.");
    }

    overlayField("overlay-safety").textContent = safety;
    overlayField("overlay-cipher").textContent = `${cipher.slice(0, 64)}…`;
    overlayField("overlay-roundtrip").textContent = accepted.plaintext;
    overlayOut.hidden = false;
    overlayNote.textContent = "Both sides agree on the safety number, and the message plus its reply decrypted.";
  } catch (e) {
    overlayNote.textContent = `The overlay module did not load: ${e}`;
  } finally {
    for (const value of allocations.reverse()) value.free();
    overlayRun.disabled = false;
  }
}

overlayRun?.addEventListener("click", runOverlay);
