// Header button: mark every chat as read.
const btn = MG.header.addButton({
  id: "read-all",
  label: "✓✓",
  title: "Mark all chats as read",
  async onClick() {
    btn.disabled = true;
    try {
      const n = await MG.invoke("mark_all_read");
      MG.toast(n ? `Marked ${n} chat${n === 1 ? "" : "s"} as read` : "Nothing unread", "success");
      MG.chats.refresh();
    } catch (e) {
      MG.toast(String(e), "error");
    } finally {
      btn.disabled = false;
    }
  },
});
return () => btn.remove();
