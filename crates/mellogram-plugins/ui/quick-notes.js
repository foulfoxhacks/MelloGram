// Per-chat private notes in a side drawer, stored in this plugin's data file.
let notes = (await MG.storage.get()) || {};
const panel = MG.h("aside", { class: "mg-drawer", hidden: true },
  MG.h("div", { class: "mg-drawer-head" }, MG.h("b", { text: "Notes" }), MG.h("button", { class: "mg-icon-btn", text: "✕", onclick: () => (panel.hidden = true) })),
  MG.h("textarea", { class: "mg-notes", placeholder: "Private notes for this chat. Saved on this device only." }));
const ta = panel.querySelector("textarea");
document.body.append(panel);
let timer;
ta.addEventListener("input", () => {
  const chat = MG.chats.current();
  if (!chat) return;
  clearTimeout(timer);
  timer = setTimeout(() => {
    if (ta.value.trim()) notes[chat.id] = ta.value; else delete notes[chat.id];
    MG.storage.set(notes);
  }, 400);
});
const load = (chat) => { ta.value = chat ? notes[chat.id] || "" : ""; };
const btn = MG.header.addButton({ id: "quick-notes", label: "📝", title: "Chat notes", onClick() {
  if (!MG.chats.current()) return MG.toast("Open a chat first", "info");
  load(MG.chats.current());
  panel.hidden = !panel.hidden;
  if (!panel.hidden) ta.focus();
} });
const off = MG.chats.onOpen(load);
const offBadge = MG.chats.onRender((chat, el) => { if (notes[chat.id]) el.querySelector(".mg-chat-name")?.append(" 📝"); });
MG.chats.rerender();
return () => { off(); offBadge(); panel.remove(); btn.remove(); };
