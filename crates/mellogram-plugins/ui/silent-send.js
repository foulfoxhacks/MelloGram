// Bell toggle in the composer. Sets `silent` on the next send.
const btn = MG.composer.addButton({
  id: "silent-send",
  label: "🔔",
  title: "Send silently (no notification)",
  onClick() { set(!MG.composer.flags.silent); },
});
function set(on) {
  MG.composer.flags.silent = on;
  btn.textContent = on ? "🔕" : "🔔";
  btn.classList.toggle("mg-active", on);
}
const off = MG.composer.onSent(() => { if (!MG.settings.persist) set(false); });
set(!!MG.settings.always);
return () => { off(); btn.remove(); delete MG.composer.flags.silent; };
