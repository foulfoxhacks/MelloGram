// Show how long each send took, next to the composer.
const el = document.createElement("div");
el.className = "mg-send-latency";
el.hidden = true;
MG.composer.footer.append(el);
const off = MG.composer.onSent((result) => {
  if (typeof result?.latency_ms !== "number") return;
  el.hidden = false;
  el.textContent = `sent in ${result.latency_ms} ms`;
  el.classList.toggle("mg-slow", result.latency_ms > (MG.settings.warn_ms || 1500));
});
return () => { off(); el.remove(); };
