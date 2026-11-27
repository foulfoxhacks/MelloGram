// Statistics over the loaded messages of the open chat.
const STOP = new Set("the a an and or to of in on for is it that this be are was i you me my we our your he she they them at with as so but if not no yes ok just do have has had what will can".split(" "));
function show() {
  const chat = MG.chats.current();
  const msgs = MG.messages.loaded();
  if (!chat || !msgs.length) return MG.toast("Open a chat with messages first", "info");
  const people = new Map(), hours = new Array(24).fill(0), words = new Map();
  let chars = 0, media = 0;
  for (const m of msgs) {
    const who = m.outgoing ? "You" : (m.display_name || m.sender_name || "Unknown");
    people.set(who, (people.get(who) || 0) + 1);
    hours[new Date(m.date * 1000).getHours()]++;
    chars += m.text.length;
    if (m.has_media) media++;
    for (const w of m.text.toLowerCase().match(/[\p{L}\p{N}']{3,}/gu) || []) if (!STOP.has(w)) words.set(w, (words.get(w) || 0) + 1);
  }
  const top = (map, n) => [...map].sort((a, b) => b[1] - a[1]).slice(0, n);
  const max = Math.max(...hours, 1);
  const first = new Date(Math.min(...msgs.map((m) => m.date)) * 1000), last = new Date(Math.max(...msgs.map((m) => m.date)) * 1000);
  MG.modal(`Statistics — ${chat.title}`, [
    MG.h("p", { class: "mg-hint", text: `${msgs.length} loaded messages from ${first.toLocaleDateString()} to ${last.toLocaleDateString()} · ${media} with media · ${Math.round(chars / msgs.length)} chars on average. Scroll up in the chat to include more history.` }),
    MG.h("h4", { text: "Messages per person" }),
    MG.h("div", { class: "mg-bars" }, ...top(people, 10).map(([k, v]) => MG.h("div", { class: "mg-bar-row" }, MG.h("span", { text: k }), MG.h("div", { class: "mg-bar", style: `width:${(v / msgs.length) * 100}%` }), MG.h("span", { text: v })))),
    MG.h("h4", { text: "Activity by hour" }),
    MG.h("div", { class: "mg-hours" }, ...hours.map((v, h) => MG.h("div", { class: "mg-hour", title: `${h}:00 — ${v}`, style: `height:${(v / max) * 100}%` }))),
    MG.h("h4", { text: "Top words" }),
    MG.h("p", { text: top(words, 25).map(([w, n]) => `${w} (${n})`).join(" · ") || "—" }),
  ]);
}
const off = MG.chats.addAction({ id: "chat-stats", label: "Statistics", onClick: (chat) => MG.chats.open(chat.id).then(show) });
const btn = MG.header.addButton({ id: "chat-stats", label: "📊", title: "Chat statistics", onClick: show });
return () => { off(); btn.remove(); };
