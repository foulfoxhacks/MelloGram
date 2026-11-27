// Live "123/4096" under the composer.
const LIMIT = 4096;
const el = document.createElement("div");
el.className = "mg-char-counter";
MG.composer.footer.append(el);
const update = (text) => {
  const n = [...text].length;
  el.hidden = n < (MG.settings.show_after || 0) || n === 0;
  el.textContent = `${n}/${LIMIT}`;
  el.classList.toggle("mg-over", n > LIMIT);
};
const off = MG.composer.onInput(update);
update(MG.composer.el.value);
return () => { off(); el.remove(); };
