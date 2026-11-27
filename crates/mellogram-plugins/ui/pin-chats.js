// Local pins: stored in this plugin's "pinned" setting, sorted to the top.
const pinned = () => new Set((MG.settings.pinned || []).map(String));
async function toggle(chat) {
  const set = pinned();
  const id = String(chat.id);
  set.has(id) ? set.delete(id) : set.add(id);
  await MG.setSetting("pinned", [...set]);
  MG.chats.rerender();
}
const offSort = MG.chats.addSorter((a, b) => {
  const p = pinned();
  return (p.has(String(b.id)) ? 1 : 0) - (p.has(String(a.id)) ? 1 : 0);
});
const offBadge = MG.chats.onRender((chat, el) => {
  el.classList.toggle("mg-local-pin", pinned().has(String(chat.id)));
});
const offAction = MG.chats.addAction({
  id: "pin-chats",
  label: (chat) => (pinned().has(String(chat.id)) ? "Unpin (local)" : "Pin (local)"),
  onClick: toggle,
});
MG.chats.rerender();
return () => { offSort(); offBadge(); offAction(); MG.chats.rerender(); };
