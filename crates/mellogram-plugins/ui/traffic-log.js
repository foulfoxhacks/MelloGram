// Traffic Log: every network exchange MelloGram makes, without its content.
// A 📡 header button opens a live table: time, direction, channel, endpoint
// (host or Telegram API method only) and size. Message text, phone numbers,
// tokens and URL paths are never recorded, so nothing sensitive is shown.
const fmt = (n) => (n >= 1048576 ? `${(n / 1048576).toFixed(1)} MB` : n >= 1024 ? `${(n / 1024).toFixed(1)} KB` : `${n} B`);
const btn = MG.header.addButton({ id: "traffic-log", label: "📡", title: "Traffic log", onClick: open });

function open() {
  const table = document.createElement("table");
  table.className = "mg-table";
  const totals = document.createElement("p");
  totals.className = "mg-hint";
  const filter = document.createElement("select");
  for (const [v, t] of [["all", "All traffic"], ["out", "Outgoing"], ["in", "Incoming"], ["https", "Web requests only"]]) {
    const o = document.createElement("option"); o.value = v; o.textContent = t; filter.append(o);
  }
  const clear = document.createElement("button");
  clear.className = "mg-btn"; clear.textContent = "Clear"; clear.type = "button";
  const bar = document.createElement("div");
  bar.className = "mg-toolbar";
  bar.append(filter, clear);
  const note = document.createElement("p");
  note.className = "mg-hint";
  note.textContent = "Recorded in memory only and redacted: hosts and API method names, never message text, numbers, tokens or links.";
  const wrap = document.createElement("div");
  wrap.className = "mg-traffic-wrap";
  wrap.append(table);
  const close = MG.ui.modal("Traffic log", [bar, totals, wrap, note]);

  async function refresh() {
    if (!wrap.isConnected) { clearInterval(timer); return; }
    let data;
    try { data = await MG.invoke("traffic_log"); } catch (e) { totals.textContent = String(e); return; }
    const f = filter.value;
    const rows = data.entries.filter((e) => f === "all" || e.dir === f || e.channel === f).slice(0, 300);
    totals.textContent = `${data.totals.entries} exchanges · sent ${fmt(data.totals.sent_bytes)} · received ${fmt(data.totals.received_bytes)}`;
    const head = "<tr><th>Time</th><th></th><th>Channel</th><th>Endpoint</th><th>Size</th><th>What</th></tr>";
    table.innerHTML = head;
    for (const e of rows) {
      const tr = document.createElement("tr");
      for (const v of [new Date(e.at * 1000).toLocaleTimeString(), e.dir === "out" ? "↑" : "↓", e.channel, e.endpoint, fmt(e.bytes), e.note]) {
        const td = document.createElement("td"); td.textContent = v; tr.append(td);
      }
      table.append(tr);
    }
    if (!rows.length) { const tr = document.createElement("tr"); const td = document.createElement("td"); td.colSpan = 6; td.textContent = "Nothing recorded yet."; tr.append(td); table.append(tr); }
  }
  filter.onchange = refresh;
  clear.onclick = async () => { await MG.invoke("traffic_clear"); refresh(); };
  const timer = setInterval(refresh, 2000);
  refresh();
  return close;
}
return () => btn.remove();
