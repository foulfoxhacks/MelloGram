// Export the loaded messages of the open chat.
function run(format) {
  const chat = MG.chats.current();
  const msgs = MG.messages.loaded();
  if (!chat || !msgs.length) return MG.toast("Open a chat with messages first", "info");
  const name = (m) => (m.outgoing ? "You" : m.display_name || m.sender_name || "Unknown");
  const when = (m) => new Date(m.date * 1000).toISOString().replace("T", " ").slice(0, 19);
  const safe = chat.title.replace(/[^\w.-]+/g, "_").slice(0, 40) || "chat";
  if (format === "json") return MG.download(`${safe}.json`, JSON.stringify({ chat, exported_at: new Date().toISOString(), messages: msgs }, null, 2), "application/json");
  if (format === "md") return MG.download(`${safe}.md`, `# ${chat.title}\n\n` + msgs.map((m) => `**${name(m)}** — _${when(m)}_${m.deleted ? " (deleted)" : ""}\n\n${m.text || "_media_"}\n`).join("\n"), "text/markdown");
  MG.download(`${safe}.txt`, msgs.map((m) => `[${when(m)}] ${name(m)}: ${m.text || "<media>"}`).join("\n"), "text/plain");
}
const off = MG.chats.addAction({ id: "chat-export", label: "Export loaded messages…", onClick: (chat) => MG.chats.open(chat.id).then(() => {
  let close;
  close = MG.modal("Export chat", [
    MG.h("p", { class: "mg-hint", text: `${MG.messages.loaded().length} messages are loaded. Scroll up first to include more.` }),
    MG.h("div", { class: "mg-toolbar" }, ...[["txt", "Plain text"], ["md", "Markdown"], ["json", "JSON"]].map(([f, l]) => MG.h("button", { class: "mg-btn", text: l, onclick: () => { run(f); close(); } }))),
  ]);
}) });
return off;
