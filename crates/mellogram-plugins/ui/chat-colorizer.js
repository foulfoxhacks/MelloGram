// Stable per-person colours from the sender id, with manual overrides.
const overrides = new Map();
for (const line of MG.settings.overrides || []) {
  const [id, color] = line.split("=>").map((s) => s.trim());
  if (id && color && CSS.supports("color", color)) overrides.set(id, color);
}
function colorFor(id) {
  if (overrides.has(String(id))) return overrides.get(String(id));
  let h = 2166136261;
  for (const c of String(id)) { h ^= c.charCodeAt(0); h = Math.imul(h, 16777619); }
  return `hsl(${Math.abs(h) % 360} ${MG.settings.saturation ?? 65}% ${MG.settings.lightness ?? 68}%)`;
}
return MG.messages.onRender((msg, el) => {
  if (msg.sender_id == null) return;
  const color = colorFor(msg.sender_id);
  const name = el.querySelector(".mg-sender");
  if (name) name.style.color = color;
  if (MG.settings.color_bubbles && !msg.outgoing) {
    el.style.background = `color-mix(in srgb, ${color} 18%, var(--mg-bubble-in))`;
  }
});
