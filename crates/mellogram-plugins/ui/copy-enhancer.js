// More ways to copy a message.
const who = (m) => (m.outgoing ? "You" : m.display_name || m.sender_name || "Unknown");
const copy = (text) => MG.clipboard.write(text).then(() => MG.toast("Copied", "success"));
const offs = [
  MG.messages.addAction({ id: "copy-quote", label: "Copy as quote", onClick: (m) => copy(m.text.split("\n").map((l) => `> ${l}`).join("\n") + `\n— ${who(m)}`) }),
  MG.messages.addAction({ id: "copy-full", label: "Copy with sender & time", onClick: (m) => copy(`[${new Date(m.date * 1000).toLocaleString()}] ${who(m)}: ${m.text}`) }),
  MG.messages.addAction({ id: "copy-link", label: "Copy message link", onClick: (m) => {
    const chat = MG.chats.current();
    if (chat?.username) return copy(`https://t.me/${chat.username}/${m.id}`);
    if (String(m.chat_id).startsWith("-100")) return copy(`https://t.me/c/${String(m.chat_id).slice(4)}/${m.id}`);
    MG.toast("Links only exist for groups and channels", "info");
  } }),
];
return () => offs.forEach((f) => f());
