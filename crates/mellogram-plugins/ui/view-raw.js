// "View raw" message action.
return MG.messages.addAction({
  id: "view-raw",
  label: "View raw",
  onClick(msg) {
    const pre = document.createElement("pre");
    pre.className = "mg-raw";
    pre.textContent = JSON.stringify(msg, null, 2);
    const copy = document.createElement("button");
    copy.textContent = "Copy";
    copy.onclick = () => navigator.clipboard.writeText(pre.textContent).then(() => MG.toast("Copied", "success"));
    MG.modal(`Message ${msg.id}`, [pre, copy]);
  },
});
