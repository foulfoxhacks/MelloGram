// All links in the loaded messages of the open chat.
function show() {
  const chat = MG.chats.current();
  if (!chat) return MG.toast("Open a chat first", "info");
  const seen = new Map();
  for (const m of MG.messages.loaded()) for (const url of m.text.match(/https?:\/\/[^\s<>"']+/g) || []) if (!seen.has(url)) seen.set(url, m);
  if (!seen.size) return MG.toast("No links in the loaded messages", "info");
  MG.modal(`Links — ${chat.title} (${seen.size})`, MG.h("div", { class: "mg-list" }, ...[...seen].reverse().map(([url, m]) => MG.h("div", { class: "mg-list-item" },
    MG.h("div", { class: "mg-list-main" },
      MG.h("a", { href: url, target: "_blank", rel: "noopener noreferrer", text: url }),
      MG.h("div", { class: "mg-hint", text: `${m.outgoing ? "You" : m.sender_name || ""} · ${new Date(m.date * 1000).toLocaleString()}` })),
    MG.h("button", { class: "mg-icon-btn", title: "Jump to message", text: "↪", onclick: () => MG.messages.jumpTo(m.id) })))));
}
const btn = MG.header.addButton({ id: "link-collection", label: "🔗", title: "Links in this chat", onClick: show });
return () => btn.remove();
