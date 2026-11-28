// Activity Tracker: how long you spend in MelloGram, per day and per chat,
// counted only while the window is focused and you have been active in the
// last two minutes. Kept on this device. Optionally sends "communicating"
// heartbeats to WakaTime (the key is stored sealed by the backend; heartbeats
// name the app, never a chat or person).
let data = (await MG.storage.get()) || { days: {} };
const today = () => new Date().toISOString().slice(0, 10);
let lastInput = Date.now();
let lastBeat = 0;
for (const ev of ["keydown", "pointerdown", "wheel"]) addEventListener(ev, onInput, { passive: true });
function onInput() { lastInput = Date.now(); }
const active = () => document.hasFocus() && Date.now() - lastInput < 120e3;

const tick = setInterval(async () => {
  if (!active()) return;
  const d = (data.days[today()] ||= { total: 0, chats: {} });
  d.total += 15;
  const chat = MG.chats.current();
  if (chat) d.chats[chat.title] = (d.chats[chat.title] || 0) + 15;
  // Keep 60 days.
  for (const k of Object.keys(data.days).sort().slice(0, -60)) delete data.days[k];
  MG.storage.set(data);
  if (MG.settings.wakatime && Date.now() - lastBeat > 120e3) {
    lastBeat = Date.now();
    MG.invoke("wakatime_heartbeat").catch((e) => MG.toast(`WakaTime: ${e}`, "error"));
  }
}, 15e3);

const dur = (s) => (s >= 3600 ? `${Math.floor(s / 3600)} h ${Math.round((s % 3600) / 60)} min` : `${Math.round(s / 60)} min`);
const btn = MG.header.addButton({ id: "activity-tracker", label: "⏱", title: "Activity", onClick: open });

async function open() {
  const box = document.createElement("div");
  const days = Object.entries(data.days).sort(([a], [b]) => b.localeCompare(a));
  const week = days.slice(0, 7).reduce((n, [, d]) => n + d.total, 0);
  const t = data.days[today()] || { total: 0, chats: {} };
  const p = (text, cls = "") => { const el = document.createElement("p"); el.textContent = text; if (cls) el.className = cls; return el; };
  box.append(p(`Today: ${dur(t.total)} · last 7 days: ${dur(week)}`));
  const top = Object.entries(t.chats).sort(([, a], [, b]) => b - a).slice(0, 8);
  if (top.length) {
    const ul = document.createElement("ul");
    for (const [name, s] of top) { const li = document.createElement("li"); li.textContent = `${name} — ${dur(s)}`; ul.append(li); }
    box.append(p("Most time today:", "mg-hint"), ul);
  }
  const status = await MG.invoke("wakatime_status").catch(() => ({ configured: false }));
  const key = document.createElement("input");
  key.type = "password"; key.placeholder = status.configured ? "Key saved — paste a new one to replace" : "waka_…"; key.autocomplete = "off";
  const save = document.createElement("button"); save.className = "mg-btn"; save.type = "button"; save.textContent = "Save key";
  const remove = document.createElement("button"); remove.className = "mg-btn"; remove.type = "button"; remove.textContent = "Remove key";
  save.onclick = async () => { try { await MG.invoke("wakatime_set_key", { key: key.value }); key.value = ""; MG.toast("WakaTime key saved (sealed on this device)", "success"); } catch (e) { MG.toast(String(e), "error"); } };
  remove.onclick = async () => { await MG.invoke("wakatime_set_key", { key: null }); MG.toast("WakaTime key removed", "info"); };
  const bar = document.createElement("div"); bar.className = "mg-toolbar"; bar.append(key, save, remove);
  box.append(p("WakaTime (optional): turn on “Send WakaTime heartbeats” in this plugin's settings and save your API key from wakatime.com/settings/api-key.", "mg-hint"), bar);
  MG.ui.modal("Activity", box);
}

return () => { clearInterval(tick); btn.remove(); for (const ev of ["keydown", "pointerdown", "wheel"]) removeEventListener(ev, onInput); };
