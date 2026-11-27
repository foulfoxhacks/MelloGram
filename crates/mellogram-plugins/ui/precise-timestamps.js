// Replace the default HH:MM message time.
const pad = (n) => String(n).padStart(2, "0");
const rtf = new Intl.RelativeTimeFormat(undefined, { numeric: "auto" });
function relative(d) {
  const s = Math.round((d - Date.now()) / 1000);
  const steps = [[60, "second"], [60, "minute"], [24, "hour"], [7, "day"], [4.35, "week"], [12, "month"], [Infinity, "year"]];
  let v = s;
  for (const [size, unit] of steps) {
    if (Math.abs(v) < size) return rtf.format(Math.round(v), unit);
    v /= size;
  }
}
const off = MG.messages.setTimeFormatter((seconds) => {
  const d = new Date(seconds * 1000);
  switch (MG.settings.format) {
    case "datetime": return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
    case "relative": return relative(d);
    default: return `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
  }
});
return off;
