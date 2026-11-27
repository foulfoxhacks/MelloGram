// Bond Watch: live toasts and a history panel.
const LABEL = { banned: "Banned", removed: "Removed", restricted: "Restricted", unbanned: "Unbanned", left: "Left", joined: "Joined", promoted: "Promoted", demoted: "Demoted", "access-lost": "Lost access" };
const unlisten = MG.onEvent?.("membership", (ev) => {
  for (const text of ev.alerts || []) MG.toast(text, ev.event.kind === "unbanned" ? "success" : "warning");
});
function describe(e) {
  const chat = e.chat_title || "a chat";
  const who = e.is_self ? "You" : (e.user_name || `User ${e.user_id}`);
  return `${who} — ${LABEL[e.kind] || e.kind} — ${chat}${e.actor_name ? ` (by ${e.actor_name})` : ""}`;
}
const btn = MG.header.addButton({ id: "bond-watch", label: "🛡", title: "Bond Watch: bans, removals and lost access", async onClick() {
  const log = await MG.invoke("membership_log");
  MG.modal(`Bond Watch (${log.length})`, log.length
    ? MG.h("div", { class: "mg-list" }, ...log.map((e) => MG.h("div", { class: "mg-list-item" },
        MG.h("span", { class: `mg-tag mg-bond-${e.kind}`, text: LABEL[e.kind] || e.kind }),
        MG.h("div", { class: "mg-list-main" },
          MG.h("b", { text: describe(e) }),
          MG.h("div", { class: "mg-hint", text: `${new Date(e.date * 1000).toLocaleString()}${e.until ? ` · until ${new Date(e.until * 1000).toLocaleString()}` : ""} · ${e.source.replace("-", " ")}` })))))
    : MG.h("p", { class: "mg-hint", text: "Nothing yet. Bond Watch records bans, removals, restrictions and chats you lose access to while MelloGram is running." }));
} });
return () => { btn.remove(); unlisten?.(); };
