// Renders the live capability list from /api/status. No dependencies, no
// storage: the page re-fetches on load and on visibility changes so it cannot
// show a stale claim about what works.
"use strict";

const list = document.getElementById("capabilities");
const version = document.getElementById("version");
const stage = document.getElementById("stage");
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
  if (data.stage) stage.textContent = data.stage;
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

async function refresh() {
  try {
    const response = await fetch("/api/status", { headers: { Accept: "application/json" } });
    if (!response.ok) throw new Error(String(response.status));
    render(await response.json());
  } catch {
    failed();
  }
}

document.addEventListener("visibilitychange", () => {
  if (document.visibilityState === "visible") refresh();
});

refresh();
