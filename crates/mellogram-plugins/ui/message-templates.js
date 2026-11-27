// Insert templates with placeholders from a composer button.
function parse() {
  return (MG.settings.templates || []).map((line) => {
    const i = line.indexOf("=>");
    return i < 0 ? null : { name: line.slice(0, i).trim(), text: line.slice(i + 2).trim().replace(/\\n/g, "\n") };
  }).filter((t) => t && t.name);
}
function fill(text) {
  const d = new Date();
  return text
    .replaceAll("{date}", d.toLocaleDateString())
    .replaceAll("{time}", d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }))
    .replaceAll("{weekday}", d.toLocaleDateString([], { weekday: "long" }))
    .replaceAll("{chat}", MG.chats.current()?.title || "");
}
const btn = MG.composer.addButton({ id: "templates", label: "📋", title: "Templates", onClick() {
  const templates = parse();
  if (!templates.length) return MG.toast("Add templates in the plugin's settings", "info");
  let close;
  close = MG.modal("Templates", MG.h("div", { class: "mg-list" }, ...templates.map((t) => MG.h("div", {
    class: "mg-list-item mg-list-main",
    onclick: () => { close(); MG.composer.insert(fill(t.text)); },
  }, MG.h("b", { text: t.name }), MG.h("div", { class: "mg-hint", text: t.text.slice(0, 140) })))));
} });
return () => btn.remove();
