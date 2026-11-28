// Send Later: a ⏰ composer button that schedules the next message with
// Telegram's own scheduled messages, so it is delivered even if MelloGram is
// closed. Pick a preset or an exact date and time; the button shows the time
// while a schedule is armed. Sending (or ✕) clears it.
const pad = (n) => String(n).padStart(2, "0");
const localInput = (d) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;

const btn = MG.composer.addButton({
  id: "send-later",
  label: "⏰",
  title: "Send later (scheduled message)",
  onClick() { MG.composer.flags.scheduleAt ? arm(null) : open(); },
});

function arm(at) {
  if (at == null) delete MG.composer.flags.scheduleAt; else MG.composer.flags.scheduleAt = at;
  btn.classList.toggle("mg-active", at != null);
  btn.textContent = at == null ? "⏰" : `⏰ ${new Date(at * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}`;
  btn.title = at == null ? "Send later (scheduled message)" : `Scheduled for ${new Date(at * 1000).toLocaleString()} — click to cancel`;
}

function open() {
  const now = new Date();
  const inOne = new Date(now.getTime() + 3600e3);
  const tomorrow9 = new Date(now); tomorrow9.setDate(now.getDate() + 1); tomorrow9.setHours(9, 0, 0, 0);
  const input = document.createElement("input");
  input.type = "datetime-local";
  input.min = localInput(new Date(now.getTime() + 60e3));
  input.max = localInput(new Date(now.getTime() + 364 * 864e5));
  input.value = localInput(inOne);
  const preset = (label, date) => {
    const b = document.createElement("button");
    b.className = "mg-btn"; b.textContent = label; b.type = "button";
    b.onclick = () => { input.value = localInput(date); };
    return b;
  };
  const presets = document.createElement("div");
  presets.className = "mg-toolbar";
  presets.append(preset("In 1 hour", inOne), preset("Tonight 20:00", (() => { const d = new Date(now); d.setHours(20, 0, 0, 0); if (d <= now) d.setDate(d.getDate() + 1); return d; })()), preset("Tomorrow 09:00", tomorrow9));
  const ok = document.createElement("button");
  ok.className = "mg-btn mg-primary"; ok.type = "button"; ok.textContent = "Schedule next message";
  const hint = document.createElement("p");
  hint.className = "mg-hint";
  hint.textContent = "Telegram delivers it at that time, even if MelloGram is closed. Scheduled messages show up in the chat once they are sent.";
  const body = document.createElement("div");
  body.append(presets, input, hint, ok);
  const close = MG.ui.modal("Send later", body);
  ok.onclick = () => {
    const at = Math.floor(new Date(input.value).getTime() / 1000);
    if (!Number.isFinite(at) || at < Date.now() / 1000 + 10) { MG.ui.toast("Pick a time in the future.", "error"); return; }
    arm(at);
    close();
    MG.composer.focus?.();
  };
}

const off = MG.composer.onSent(() => arm(null));
return () => { off(); btn.remove(); delete MG.composer.flags.scheduleAt; };
