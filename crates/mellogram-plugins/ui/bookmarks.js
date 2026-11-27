// Bookmark messages; list and jump from the header.
let marks = (await MG.storage.get()) || [];
const save = () => MG.storage.set(marks);
const offAction = MG.messages.addAction({ id: "bookmark", label: "Bookmark", onClick(msg) {
  if (marks.some((b) => b.chat_id === msg.chat_id && b.id === msg.id)) return MG.toast("Already bookmarked", "info");
  const chat = MG.chats.current();
  marks.unshift({ chat_id: msg.chat_id, id: msg.id, chat: chat?.title || "", sender: msg.display_name || msg.sender_name || "", text: msg.text.slice(0, 300), date: msg.date, saved: Date.now() });
  save();
  MG.toast("Bookmarked", "success");
} });
const btn = MG.header.addButton({ id: "bookmarks", label: "🔖", title: "Bookmarks", onClick() {
  if (!marks.length) return MG.toast("No bookmarks yet — right-click a message → Bookmark", "info");
  let close;
  const list = MG.h("div", { class: "mg-list" }, ...marks.map((b, i) => MG.h("div", { class: "mg-list-item" },
    MG.h("div", { class: "mg-list-main", onclick: async () => { close(); await MG.chats.open(b.chat_id); MG.messages.jumpTo(b.id); } },
      MG.h("b", { text: `${b.chat} · ${b.sender}` }),
      MG.h("div", { class: "mg-hint", text: new Date(b.date * 1000).toLocaleString() }),
      MG.h("div", { text: b.text || "Media" })),
    MG.h("button", { class: "mg-icon-btn", title: "Remove", text: "🗑", onclick: (e) => { e.stopPropagation(); marks.splice(i, 1); save(); e.target.closest(".mg-list-item").remove(); } }))));
  close = MG.modal(`Bookmarks (${marks.length})`, list);
} });
return () => { offAction(); btn.remove(); };
