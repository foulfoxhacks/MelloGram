// Global Badges: people who publish a public MelloID profile get their
// badges shown next to their name in chats, in every MelloGram install.
// Lookups go through the backend (user ids only, batched, cached 30 min);
// unlisted and private profiles are never matched.
const cache = new Map(); // user id -> { at, data }
const pending = new Set();
let timer = null;

function queue(id) {
  const hit = cache.get(id);
  if ((hit && Date.now() - hit.at < 30 * 60e3) || pending.has(id)) return;
  pending.add(id);
  clearTimeout(timer);
  timer = setTimeout(flush, 400);
}

async function flush() {
  const ids = [...pending].slice(0, 50);
  ids.forEach((id) => pending.delete(id));
  if (!ids.length) return;
  try {
    const users = await MG.invoke("melloid_badges", { ids: ids.map(Number) });
    for (const id of ids) cache.set(id, { at: Date.now(), data: users[id] || null });
    MG.messages.rerender?.();
  } catch (e) {
    for (const id of ids) cache.set(id, { at: Date.now(), data: null });
    console.warn("global badges", e);
  }
  if (pending.size) flush();
}

const off = MG.messages.onRender((m, el) => {
  const id = String(m.sender_id ?? "");
  if (!/^[1-9][0-9]*$/.test(id)) return;
  const hit = cache.get(id);
  if (!hit) { queue(id); return; }
  if (!hit.data) return;
  const name = el.querySelector(".mg-sender");
  if (!name || name.querySelector(".mg-global-badge")) return;
  const a = document.createElement("a");
  a.className = "mg-global-badge";
  a.href = `https://id.mellogram.foulfoxhacks.dev/@/${encodeURIComponent(hit.data.slug)}`;
  a.target = "_blank"; a.rel = "noopener noreferrer";
  a.title = ["MelloID profile", ...hit.data.badges.map((b) => b.title)].join(" · ");
  a.textContent = hit.data.badges.length ? `✦ ${hit.data.badges[0].title}${hit.data.badges.length > 1 ? ` +${hit.data.badges.length - 1}` : ""}` : "✦ MelloID";
  name.append(a);
});
return () => { off(); clearTimeout(timer); };
